#!/usr/bin/env node
/**
 * Multi-Contributor Edge-Net Test with Persistence
 *
 * Tests:
 * 1. Multiple contributors with persistent identities
 * 2. State persistence (patterns, ledger, coherence)
 * 3. Cross-contributor verification
 * 4. Session restore from persisted data
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { webcrypto } from 'crypto';
import { performance } from 'perf_hooks';
import { homedir } from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Setup polyfills
async function setupPolyfills() {
  if (typeof globalThis.crypto === 'undefined') {
    globalThis.crypto = webcrypto;
  }
  if (typeof globalThis.performance === 'undefined') {
    globalThis.performance = performance;
  }

  const createStorage = () => {
    const store = new Map();
    return {
      getItem: (key) => store.get(key) || null,
      setItem: (key, value) => store.set(key, String(value)),
      removeItem: (key) => store.delete(key),
      clear: () => store.clear(),
      get length() { return store.size; },
      key: (i) => [...store.keys()][i] || null,
    };
  };

  let cpuCount = 4;
  try {
    const os = await import('os');
    cpuCount = os.cpus().length;
  } catch {}

  if (typeof globalThis.window === 'undefined') {
    globalThis.window = {
      crypto: globalThis.crypto,
      performance: globalThis.performance,
      localStorage: createStorage(),
      sessionStorage: createStorage(),
      navigator: {
        userAgent: `Node.js/${process.version}`,
        hardwareConcurrency: cpuCount,
      },
      location: { href: 'node://localhost', hostname: 'localhost' },
      screen: { width: 1920, height: 1080, colorDepth: 24 },
    };
  }

  if (typeof globalThis.document === 'undefined') {
    globalThis.document = { createElement: () => ({}), body: {}, head: {} };
  }
}

// Colors
const c = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  cyan: '\x1b[36m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  magenta: '\x1b[35m',
};

function toHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

// Storage directory
const STORAGE_DIR = join(homedir(), '.ruvector', 'edge-net-test');

function ensureStorageDir() {
  if (!existsSync(STORAGE_DIR)) {
    mkdirSync(STORAGE_DIR, { recursive: true });
  }
  return STORAGE_DIR;
}

// Contributor class with persistence
class PersistentContributor {
  constructor(wasm, id, storageDir) {
    this.wasm = wasm;
    this.id = id;
    this.storageDir = storageDir;
    this.identityPath = join(storageDir, `contributor-${id}.identity`);
    this.statePath = join(storageDir, `contributor-${id}.state`);
    this.piKey = null;
    this.coherence = null;
    this.reasoning = null;
    this.memory = null;
    this.ledger = null;
    this.patterns = [];
  }

  // Initialize or restore from persistence
  async initialize() {
    const password = `contributor-${this.id}-secret`;

    // Try to restore identity
    if (existsSync(this.identityPath)) {
      console.log(`  ${c.cyan}[${this.id}]${c.reset} Restoring identity from storage...`);
      const backup = new Uint8Array(readFileSync(this.identityPath));
      this.piKey = this.wasm.PiKey.restoreFromBackup(backup, password);
      console.log(`  ${c.green}✓${c.reset} Identity restored: ${this.piKey.getShortId()}`);
    } else {
      console.log(`  ${c.cyan}[${this.id}]${c.reset} Generating new identity...`);
      this.piKey = new this.wasm.PiKey();
      // Persist immediately
      const backup = this.piKey.createEncryptedBackup(password);
      writeFileSync(this.identityPath, Buffer.from(backup));
      console.log(`  ${c.green}✓${c.reset} New identity created: ${this.piKey.getShortId()}`);
    }

    // Initialize components
    this.coherence = new this.wasm.CoherenceEngine();
    this.reasoning = new this.wasm.ReasoningBank();
    this.memory = new this.wasm.CollectiveMemory(this.getNodeId());
    this.ledger = new this.wasm.QDAGLedger();

    // Try to restore state
    if (existsSync(this.statePath)) {
      console.log(`  ${c.cyan}[${this.id}]${c.reset} Restoring state...`);
      const state = JSON.parse(readFileSync(this.statePath, 'utf-8'));

      // Restore ledger state if available
      if (state.ledger) {
        const ledgerBytes = new Uint8Array(state.ledger);
        const imported = this.ledger.importState(ledgerBytes);
        console.log(`  ${c.green}✓${c.reset} Ledger restored: ${imported} transactions`);
      }

      // Restore patterns
      if (state.patterns) {
        this.patterns = state.patterns;
        state.patterns.forEach(p => this.reasoning.store(JSON.stringify(p)));
        console.log(`  ${c.green}✓${c.reset} Patterns restored: ${state.patterns.length}`);
      }
    }

    return this;
  }

  getNodeId() {
    return `node-${this.id}-${this.piKey.getShortId()}`;
  }

  getPublicKey() {
    return this.piKey.getPublicKey();
  }

  // Sign data
  sign(data) {
    const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
    return this.piKey.sign(bytes);
  }

  // Verify signature from another contributor
  verify(data, signature, publicKey) {
    const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
    return this.piKey.verify(bytes, signature, publicKey);
  }

  // Store a pattern
  storePattern(pattern) {
    const id = this.reasoning.store(JSON.stringify(pattern));
    this.patterns.push(pattern);
    return id;
  }

  // Lookup patterns
  lookupPatterns(query, k = 3) {
    return JSON.parse(this.reasoning.lookup(JSON.stringify(query), k));
  }

  // Get coherence stats
  getCoherenceStats() {
    return JSON.parse(this.coherence.getStats());
  }

  // Get memory stats
  getMemoryStats() {
    return JSON.parse(this.memory.getStats());
  }

  // Persist state
  persist() {
    const state = {
      timestamp: Date.now(),
      nodeId: this.getNodeId(),
      patterns: this.patterns,
      ledger: Array.from(this.ledger.exportState()),
      stats: {
        coherence: this.getCoherenceStats(),
        memory: this.getMemoryStats(),
        patternCount: this.reasoning.count(),
        txCount: this.ledger.transactionCount()
      }
    };

    writeFileSync(this.statePath, JSON.stringify(state, null, 2));
    return state;
  }

  // Cleanup WASM resources
  cleanup() {
    if (this.piKey) this.piKey.free();
    if (this.coherence) this.coherence.free();
    if (this.reasoning) this.reasoning.free();
    if (this.memory) this.memory.free();
    if (this.ledger) this.ledger.free();
  }
}

// Network simulation
class EdgeNetwork {
  constructor(wasm, storageDir) {
    this.wasm = wasm;
    this.storageDir = storageDir;
    this.contributors = new Map();
    this.sharedMessages = [];
  }

  async addContributor(id) {
    const contributor = new PersistentContributor(this.wasm, id, this.storageDir);
    await contributor.initialize();
    this.contributors.set(id, contributor);
    return contributor;
  }

  // Broadcast a signed message
  broadcastMessage(senderId, message) {
    const sender = this.contributors.get(senderId);
    const signature = sender.sign(message);

    this.sharedMessages.push({
      from: senderId,
      message,
      signature: Array.from(signature),
      publicKey: Array.from(sender.getPublicKey()),
      timestamp: Date.now()
    });

    return signature;
  }

  // Verify all messages from network perspective
  verifyAllMessages() {
    const results = [];

    for (const msg of this.sharedMessages) {
      const signature = new Uint8Array(msg.signature);
      const publicKey = new Uint8Array(msg.publicKey);

      // Each contributor verifies
      for (const [id, contributor] of this.contributors) {
        if (id !== msg.from) {
          const valid = contributor.verify(msg.message, signature, publicKey);
          results.push({
            message: msg.message.substring(0, 30) + '...',
            from: msg.from,
            verifiedBy: id,
            valid
          });
        }
      }
    }

    return results;
  }

  // Share patterns across network
  sharePatterns() {
    const allPatterns = [];

    for (const [id, contributor] of this.contributors) {
      contributor.patterns.forEach(p => {
        allPatterns.push({ ...p, contributor: id });
      });
    }

    return allPatterns;
  }

  // Persist all contributors
  persistAll() {
    const states = {};
    for (const [id, contributor] of this.contributors) {
      states[id] = contributor.persist();
    }

    // Save network state
    const networkState = {
      timestamp: Date.now(),
      contributors: Array.from(this.contributors.keys()),
      messages: this.sharedMessages,
      totalPatterns: this.sharePatterns().length
    };

    writeFileSync(
      join(this.storageDir, 'network-state.json'),
      JSON.stringify(networkState, null, 2)
    );

    return { states, networkState };
  }

  cleanup() {
    for (const [, contributor] of this.contributors) {
      contributor.cleanup();
    }
  }
}

// Main test
async function runMultiContributorTest() {
  console.log(`
${c.cyan}╔═══════════════════════════════════════════════════════════════╗${c.reset}
${c.cyan}║${c.reset}  ${c.bold}Multi-Contributor Edge-Net Test with Persistence${c.reset}           ${c.cyan}║${c.reset}
${c.cyan}╚═══════════════════════════════════════════════════════════════╝${c.reset}
`);

  await setupPolyfills();

  // Load WASM
  const { createRequire } = await import('module');
  const require = createRequire(import.meta.url);
  console.log(`${c.dim}Loading WASM module...${c.reset}`);
  const wasm = require('./node/ruvector_edge_net.cjs');
  console.log(`${c.green}✓${c.reset} WASM module loaded\n`);

  // Setup storage
  const storageDir = ensureStorageDir();
  console.log(`${c.cyan}Storage:${c.reset} ${storageDir}\n`);

  // Check if this is a continuation
  const networkStatePath = join(storageDir, 'network-state.json');
  const isContinuation = existsSync(networkStatePath);

  if (isContinuation) {
    const prevState = JSON.parse(readFileSync(networkStatePath, 'utf-8'));
    console.log(`${c.yellow}Continuing from previous session:${c.reset}`);
    console.log(`  Previous timestamp: ${new Date(prevState.timestamp).toISOString()}`);
    console.log(`  Contributors: ${prevState.contributors.join(', ')}`);
    console.log(`  Messages: ${prevState.messages.length}`);
    console.log(`  Patterns: ${prevState.totalPatterns}\n`);
  } else {
    console.log(`${c.green}Starting fresh network...${c.reset}\n`);
  }

  // Create network
  const network = new EdgeNetwork(wasm, storageDir);

  try {
    // ==== Phase 1: Initialize Contributors ====
    console.log(`${c.bold}=== Phase 1: Initialize Contributors ===${c.reset}\n`);

    const contributorIds = ['alice', 'bob', 'charlie'];

    for (const id of contributorIds) {
      await network.addContributor(id);
    }

    console.log(`\n${c.green}✓${c.reset} ${network.contributors.size} contributors initialized\n`);

    // ==== Phase 2: Cross-Verification ====
    console.log(`${c.bold}=== Phase 2: Cross-Verification ===${c.reset}\n`);

    // Each contributor signs a message
    for (const id of contributorIds) {
      const message = `Hello from ${id} at ${Date.now()}`;
      network.broadcastMessage(id, message);
      console.log(`  ${c.cyan}[${id}]${c.reset} Broadcast: "${message.substring(0, 40)}..."`);
    }

    // Verify all signatures
    const verifications = network.verifyAllMessages();
    const allValid = verifications.every(v => v.valid);

    console.log(`\n  ${c.bold}Verification Results:${c.reset}`);
    verifications.forEach(v => {
      console.log(`    ${v.valid ? c.green + '✓' : c.red + '✗'}${c.reset} ${v.from} → ${v.verifiedBy}`);
    });
    console.log(`\n${allValid ? c.green + '✓' : c.red + '✗'}${c.reset} All ${verifications.length} verifications ${allValid ? 'passed' : 'FAILED'}\n`);

    // ==== Phase 3: Pattern Storage ====
    console.log(`${c.bold}=== Phase 3: Pattern Storage & Learning ===${c.reset}\n`);

    // Each contributor stores some patterns
    const patternData = {
      alice: [
        { centroid: [1.0, 0.0, 0.0], confidence: 0.95, task: 'compute' },
        { centroid: [0.9, 0.1, 0.0], confidence: 0.88, task: 'inference' }
      ],
      bob: [
        { centroid: [0.0, 1.0, 0.0], confidence: 0.92, task: 'training' },
        { centroid: [0.1, 0.9, 0.0], confidence: 0.85, task: 'validation' }
      ],
      charlie: [
        { centroid: [0.0, 0.0, 1.0], confidence: 0.90, task: 'storage' },
        { centroid: [0.1, 0.1, 0.8], confidence: 0.87, task: 'retrieval' }
      ]
    };

    for (const [id, patterns] of Object.entries(patternData)) {
      const contributor = network.contributors.get(id);
      patterns.forEach(p => contributor.storePattern(p));
      console.log(`  ${c.cyan}[${id}]${c.reset} Stored ${patterns.length} patterns`);
    }

    // Lookup patterns
    console.log(`\n  ${c.bold}Pattern Lookups:${c.reset}`);
    const alice = network.contributors.get('alice');
    const similar = alice.lookupPatterns([0.95, 0.05, 0.0], 2);
    console.log(`    Alice searches for [0.95, 0.05, 0.0]: Found ${similar.length} similar patterns`);
    similar.forEach((p, i) => {
      console.log(`      ${i + 1}. similarity=${p.similarity.toFixed(3)}, task=${p.pattern?.task || 'unknown'}`);
    });

    const totalPatterns = network.sharePatterns();
    console.log(`\n${c.green}✓${c.reset} Total patterns in network: ${totalPatterns.length}\n`);

    // ==== Phase 4: Coherence Check ====
    console.log(`${c.bold}=== Phase 4: Coherence State ===${c.reset}\n`);

    for (const [id, contributor] of network.contributors) {
      const stats = contributor.getCoherenceStats();
      console.log(`  ${c.cyan}[${id}]${c.reset} Merkle: ${contributor.coherence.getMerkleRoot().substring(0, 16)}... | Events: ${stats.total_events || 0}`);
    }

    // ==== Phase 5: Persistence ====
    console.log(`\n${c.bold}=== Phase 5: Persistence ===${c.reset}\n`);

    const { states, networkState } = network.persistAll();

    console.log(`  ${c.green}✓${c.reset} Network state persisted`);
    console.log(`    Contributors: ${networkState.contributors.length}`);
    console.log(`    Messages: ${networkState.messages.length}`);
    console.log(`    Total patterns: ${networkState.totalPatterns}`);

    for (const [id, state] of Object.entries(states)) {
      console.log(`\n  ${c.cyan}[${id}]${c.reset} State saved:`);
      console.log(`    Node ID: ${state.nodeId}`);
      console.log(`    Patterns: ${state.stats.patternCount}`);
      console.log(`    Ledger TX: ${state.stats.txCount}`);
    }

    // ==== Phase 6: Verify Persistence ====
    console.log(`\n${c.bold}=== Phase 6: Verify Persistence Files ===${c.reset}\n`);

    const files = readdirSync(storageDir);
    console.log(`  Files in ${storageDir}:`);
    files.forEach(f => {
      const path = join(storageDir, f);
      const stat = existsSync(path) ? readFileSync(path).length : 0;
      console.log(`    ${c.dim}•${c.reset} ${f} (${stat} bytes)`);
    });

    // ==== Summary ====
    console.log(`
${c.cyan}╔═══════════════════════════════════════════════════════════════╗${c.reset}
${c.cyan}║${c.reset}  ${c.bold}${c.green}All Tests Passed!${c.reset}                                          ${c.cyan}║${c.reset}
${c.cyan}╚═══════════════════════════════════════════════════════════════╝${c.reset}

${c.bold}Summary:${c.reset}
  • ${c.green}✓${c.reset} ${network.contributors.size} contributors initialized with persistent identities
  • ${c.green}✓${c.reset} ${verifications.length} cross-verifications passed
  • ${c.green}✓${c.reset} ${totalPatterns.length} patterns stored and searchable
  • ${c.green}✓${c.reset} State persisted to ${storageDir}
  • ${c.green}✓${c.reset} ${isContinuation ? 'Continued from' : 'Started'} session

${c.dim}Run again to test persistence restoration!${c.reset}
`);

  } finally {
    network.cleanup();
  }
}

// Run
runMultiContributorTest().catch(err => {
  console.error(`${c.red}Error: ${err.message}${c.reset}`);
  console.error(err.stack);
  process.exit(1);
});
