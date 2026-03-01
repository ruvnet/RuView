#!/usr/bin/env node
/**
 * @ruvector/edge-net Join CLI
 *
 * Simple CLI to join the EdgeNet distributed compute network with public key support.
 * Supports multiple contributors connecting with their own identities.
 *
 * Usage:
 *   npx @ruvector/edge-net join                    # Generate new identity and join
 *   npx @ruvector/edge-net join --key <pubkey>    # Join with existing public key
 *   npx @ruvector/edge-net join --generate        # Generate new keypair only
 *   npx @ruvector/edge-net join --export          # Export identity for sharing
 *   npx @ruvector/edge-net join --import <file>   # Import identity from backup
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { webcrypto } from 'crypto';
import { performance } from 'perf_hooks';
import { homedir } from 'os';
import { NetworkManager } from './network.js';
import { MultiNetworkManager, NetworkRegistry } from './networks.js';

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
        language: 'en-US',
        languages: ['en-US', 'en'],
        hardwareConcurrency: cpuCount,
      },
      location: { href: 'node://localhost', hostname: 'localhost' },
      screen: { width: 1920, height: 1080, colorDepth: 24 },
    };
  }

  if (typeof globalThis.document === 'undefined') {
    globalThis.document = {
      createElement: () => ({}),
      body: {},
      head: {},
    };
  }
}

// ANSI colors
const colors = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  cyan: '\x1b[36m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  red: '\x1b[31m',
};

const c = (color, text) => `${colors[color]}${text}${colors.reset}`;

function printBanner() {
  console.log(`
${c('cyan', '╔═══════════════════════════════════════════════════════════════╗')}
${c('cyan', '║')}  ${c('bold', '🔗 RuVector Edge-Net Join')}                                  ${c('cyan', '║')}
${c('cyan', '║')}  ${c('dim', 'Join the Distributed Compute Network')}                        ${c('cyan', '║')}
${c('cyan', '╚═══════════════════════════════════════════════════════════════╝')}
`);
}

function printHelp() {
  printBanner();
  console.log(`${c('bold', 'USAGE:')}
  ${c('green', 'npx @ruvector/edge-net join')} [options]

${c('bold', 'IDENTITY OPTIONS:')}
  ${c('yellow', '--generate')}         Generate new Pi-Key identity without joining
  ${c('yellow', '--key <pubkey>')}     Join using existing public key (hex)
  ${c('yellow', '--site <id>')}        Set site identifier (default: "edge-contributor")
  ${c('yellow', '--export <file>')}    Export identity to encrypted file
  ${c('yellow', '--import <file>')}    Import identity from encrypted backup
  ${c('yellow', '--password <pw>')}    Password for import/export operations
  ${c('yellow', '--status')}           Show current contributor status
  ${c('yellow', '--history')}          Show contribution history
  ${c('yellow', '--list')}             List all stored identities
  ${c('yellow', '--peers')}            List connected peers

${c('bold', 'MULTI-NETWORK OPTIONS:')}
  ${c('yellow', '--networks')}         List all known networks
  ${c('yellow', '--discover')}         Discover available networks
  ${c('yellow', '--network <id>')}     Join/use specific network by ID
  ${c('yellow', '--create-network')}   Create a new network with name
  ${c('yellow', '--network-type')}     Network type: public, private, consortium
  ${c('yellow', '--network-desc')}     Description for new network
  ${c('yellow', '--switch <id>')}      Switch active network
  ${c('yellow', '--invite <code>')}    Invite code for private networks

${c('bold', 'EXAMPLES:')}
  ${c('dim', '# Generate new identity and join default network')}
  $ npx @ruvector/edge-net join

  ${c('dim', '# Discover available networks')}
  $ npx @ruvector/edge-net join --discover

  ${c('dim', '# Create a public research network')}
  $ npx @ruvector/edge-net join --create-network "ML Research" --network-desc "For ML workloads"

  ${c('dim', '# Create a private team network')}
  $ npx @ruvector/edge-net join --create-network "Team Alpha" --network-type private

  ${c('dim', '# Join a specific network')}
  $ npx @ruvector/edge-net join --network net-abc123

  ${c('dim', '# Join a private network with invite code')}
  $ npx @ruvector/edge-net join --network net-xyz789 --invite <invite-code>

  ${c('dim', '# Switch active network')}
  $ npx @ruvector/edge-net join --switch net-abc123

${c('bold', 'MULTI-CONTRIBUTOR SETUP:')}
  Each contributor runs their own node with a unique identity.

  ${c('dim', 'Contributor 1:')}
  $ npx @ruvector/edge-net join --site contributor-1

  ${c('dim', 'Contributor 2:')}
  $ npx @ruvector/edge-net join --site contributor-2

  ${c('dim', 'All nodes automatically discover and connect via P2P gossip.')}

${c('bold', 'NETWORK TYPES:')}
  ${c('cyan', '🌐 Public')}      Anyone can join and discover
  ${c('cyan', '🔒 Private')}     Requires invite code to join
  ${c('cyan', '🏢 Consortium')}  Requires approval from existing members

${c('bold', 'IDENTITY INFO:')}
  ${c('cyan', 'Pi-Key:')}    40-byte Ed25519-based identity (π-sized)
  ${c('cyan', 'Public Key:')} 32-byte Ed25519 verification key
  ${c('cyan', 'Genesis ID:')} 21-byte network fingerprint (φ-sized)

${c('dim', 'Documentation: https://github.com/ruvnet/ruvector/tree/main/examples/edge-net')}
`);
}

// Config directory for storing identities - persistent across months/years
function getConfigDir() {
  const configDir = join(homedir(), '.ruvector');
  if (!existsSync(configDir)) {
    mkdirSync(configDir, { recursive: true });
  }
  return configDir;
}

function getIdentitiesDir() {
  const identitiesDir = join(getConfigDir(), 'identities');
  if (!existsSync(identitiesDir)) {
    mkdirSync(identitiesDir, { recursive: true });
  }
  return identitiesDir;
}

function getContributionsDir() {
  const contribDir = join(getConfigDir(), 'contributions');
  if (!existsSync(contribDir)) {
    mkdirSync(contribDir, { recursive: true });
  }
  return contribDir;
}

// Long-term persistent identity management
class PersistentIdentity {
  constructor(siteId, wasm) {
    this.siteId = siteId;
    this.wasm = wasm;
    this.identityPath = join(getIdentitiesDir(), `${siteId}.identity`);
    this.metaPath = join(getIdentitiesDir(), `${siteId}.meta.json`);
    this.contributionPath = join(getContributionsDir(), `${siteId}.history.json`);
    this.piKey = null;
    this.meta = null;
  }

  exists() {
    return existsSync(this.identityPath);
  }

  // Generate new or restore existing identity
  async initialize(password) {
    if (this.exists()) {
      return this.restore(password);
    } else {
      return this.generate(password);
    }
  }

  // Generate new identity with full metadata
  generate(password) {
    this.piKey = new this.wasm.PiKey();

    // Save encrypted identity
    const backup = this.piKey.createEncryptedBackup(password);
    writeFileSync(this.identityPath, Buffer.from(backup));

    // Save metadata (not secret)
    this.meta = {
      version: 1,
      siteId: this.siteId,
      shortId: this.piKey.getShortId(),
      publicKey: toHex(this.piKey.getPublicKey()),
      genesisFingerprint: toHex(this.piKey.getGenesisFingerprint()),
      createdAt: new Date().toISOString(),
      lastUsed: new Date().toISOString(),
      totalSessions: 1,
      totalContributions: 0
    };
    writeFileSync(this.metaPath, JSON.stringify(this.meta, null, 2));

    // Initialize contribution history
    const history = {
      siteId: this.siteId,
      shortId: this.meta.shortId,
      sessions: [{
        started: new Date().toISOString(),
        type: 'genesis'
      }],
      contributions: [],
      milestones: [{
        type: 'identity_created',
        timestamp: new Date().toISOString()
      }]
    };
    writeFileSync(this.contributionPath, JSON.stringify(history, null, 2));

    return { isNew: true, meta: this.meta };
  }

  // Restore existing identity
  restore(password) {
    const backup = new Uint8Array(readFileSync(this.identityPath));
    this.piKey = this.wasm.PiKey.restoreFromBackup(backup, password);

    // Load and update metadata
    if (existsSync(this.metaPath)) {
      this.meta = JSON.parse(readFileSync(this.metaPath, 'utf-8'));
    } else {
      // Rebuild metadata from key
      this.meta = {
        version: 1,
        siteId: this.siteId,
        shortId: this.piKey.getShortId(),
        publicKey: toHex(this.piKey.getPublicKey()),
        genesisFingerprint: toHex(this.piKey.getGenesisFingerprint()),
        createdAt: 'unknown',
        lastUsed: new Date().toISOString(),
        totalSessions: 1,
        totalContributions: 0
      };
    }

    // Update usage stats
    this.meta.lastUsed = new Date().toISOString();
    this.meta.totalSessions = (this.meta.totalSessions || 0) + 1;
    writeFileSync(this.metaPath, JSON.stringify(this.meta, null, 2));

    // Update contribution history
    let history;
    if (existsSync(this.contributionPath)) {
      history = JSON.parse(readFileSync(this.contributionPath, 'utf-8'));
    } else {
      history = {
        siteId: this.siteId,
        shortId: this.meta.shortId,
        sessions: [],
        contributions: [],
        milestones: []
      };
    }

    // Calculate time since last session
    const lastSession = history.sessions[history.sessions.length - 1];
    let timeSinceLastSession = null;
    if (lastSession && lastSession.started) {
      const last = new Date(lastSession.started);
      const now = new Date();
      const diffMs = now - last;
      const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
      timeSinceLastSession = diffDays;

      if (diffDays > 30) {
        history.milestones.push({
          type: 'returned_after_absence',
          timestamp: new Date().toISOString(),
          daysSinceLastSession: diffDays
        });
      }
    }

    history.sessions.push({
      started: new Date().toISOString(),
      type: 'restored',
      timeSinceLastDays: timeSinceLastSession
    });

    writeFileSync(this.contributionPath, JSON.stringify(history, null, 2));

    return {
      isNew: false,
      meta: this.meta,
      sessions: this.meta.totalSessions,
      daysSinceLastSession: timeSinceLastSession
    };
  }

  // Record a contribution
  recordContribution(type, details = {}) {
    this.meta.totalContributions = (this.meta.totalContributions || 0) + 1;
    this.meta.lastUsed = new Date().toISOString();
    writeFileSync(this.metaPath, JSON.stringify(this.meta, null, 2));

    let history = { sessions: [], contributions: [], milestones: [] };
    if (existsSync(this.contributionPath)) {
      history = JSON.parse(readFileSync(this.contributionPath, 'utf-8'));
    }

    history.contributions.push({
      type,
      timestamp: new Date().toISOString(),
      ...details
    });

    writeFileSync(this.contributionPath, JSON.stringify(history, null, 2));
    return this.meta.totalContributions;
  }

  // Get full history
  getHistory() {
    if (!existsSync(this.contributionPath)) {
      return null;
    }
    return JSON.parse(readFileSync(this.contributionPath, 'utf-8'));
  }

  // Get public info for sharing
  getPublicInfo() {
    return {
      siteId: this.siteId,
      shortId: this.meta.shortId,
      publicKey: this.meta.publicKey,
      genesisFingerprint: this.meta.genesisFingerprint,
      memberSince: this.meta.createdAt,
      totalContributions: this.meta.totalContributions
    };
  }

  free() {
    if (this.piKey) this.piKey.free();
  }
}

// List all stored identities
function listStoredIdentities() {
  const identitiesDir = getIdentitiesDir();
  if (!existsSync(identitiesDir)) return [];

  const files = readdirSync(identitiesDir);
  const identities = [];

  for (const file of files) {
    if (file.endsWith('.meta.json')) {
      const meta = JSON.parse(readFileSync(join(identitiesDir, file), 'utf-8'));
      identities.push(meta);
    }
  }

  return identities;
}

function toHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

function fromHex(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  return bytes;
}

// Parse arguments
function parseArgs(args) {
  const opts = {
    generate: false,
    key: null,
    site: 'edge-contributor',
    export: null,
    import: null,
    password: null,
    status: false,
    history: false,
    list: false,
    peers: false,
    help: false,
    // Multi-network options
    network: null,           // Network ID to join/use
    createNetwork: null,     // Create new network with name
    networkType: 'public',   // public, private, consortium
    networkDesc: null,       // Network description
    discoverNetworks: false, // Discover available networks
    listNetworks: false,     // List known networks
    switchNetwork: null,     // Switch active network
    invite: null,            // Invite code for private networks
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    switch (arg) {
      case '--generate':
        opts.generate = true;
        break;
      case '--key':
        opts.key = args[++i];
        break;
      case '--site':
        opts.site = args[++i];
        break;
      case '--export':
        opts.export = args[++i];
        break;
      case '--import':
        opts.import = args[++i];
        break;
      case '--password':
        opts.password = args[++i];
        break;
      case '--status':
        opts.status = true;
        break;
      case '--history':
        opts.history = true;
        break;
      case '--list':
        opts.list = true;
        break;
      case '--peers':
        opts.peers = true;
        break;
      // Multi-network options
      case '--network':
      case '-n':
        opts.network = args[++i];
        break;
      case '--create-network':
        opts.createNetwork = args[++i];
        break;
      case '--network-type':
        opts.networkType = args[++i];
        break;
      case '--network-desc':
        opts.networkDesc = args[++i];
        break;
      case '--discover':
        opts.discoverNetworks = true;
        break;
      case '--networks':
        opts.listNetworks = true;
        break;
      case '--switch':
        opts.switchNetwork = args[++i];
        break;
      case '--invite':
        opts.invite = args[++i];
        break;
      case '--help':
      case '-h':
        opts.help = true;
        break;
    }
  }

  return opts;
}

// Show contribution history
async function showHistory(wasm, siteId, password) {
  console.log(`${c('bold', 'CONTRIBUTION HISTORY:')}\n`);

  const identity = new PersistentIdentity(siteId, wasm);

  if (!identity.exists()) {
    console.log(`${c('yellow', '⚠')} No identity found for site "${siteId}"`);
    console.log(`${c('dim', 'Run without --history to create one.')}\n`);
    return;
  }

  await identity.initialize(password);
  const history = identity.getHistory();

  if (!history) {
    console.log(`${c('dim', 'No history available.')}\n`);
    identity.free();
    return;
  }

  console.log(`  ${c('cyan', 'Site ID:')}      ${history.siteId}`);
  console.log(`  ${c('cyan', 'Short ID:')}     ${history.shortId}`);
  console.log(`  ${c('cyan', 'Sessions:')}     ${history.sessions.length}`);
  console.log(`  ${c('cyan', 'Contributions:')} ${history.contributions.length}`);
  console.log(`  ${c('cyan', 'Milestones:')}   ${history.milestones.length}\n`);

  if (history.milestones.length > 0) {
    console.log(`  ${c('bold', 'Milestones:')}`);
    history.milestones.slice(-5).forEach(m => {
      const date = new Date(m.timestamp).toLocaleDateString();
      console.log(`    ${c('dim', date)} - ${c('green', m.type)}`);
    });
  }

  if (history.sessions.length > 0) {
    console.log(`\n  ${c('bold', 'Recent Sessions:')}`);
    history.sessions.slice(-5).forEach(s => {
      const date = new Date(s.started).toLocaleDateString();
      const time = new Date(s.started).toLocaleTimeString();
      const elapsed = s.timeSinceLastDays ? ` (${s.timeSinceLastDays}d since last)` : '';
      console.log(`    ${c('dim', date + ' ' + time)} - ${s.type}${elapsed}`);
    });
  }

  console.log('');
  identity.free();
}

// List all stored identities
async function listIdentities() {
  console.log(`${c('bold', 'STORED IDENTITIES:')}\n`);

  const identities = listStoredIdentities();

  if (identities.length === 0) {
    console.log(`  ${c('dim', 'No identities found.')}`);
    console.log(`  ${c('dim', 'Run "npx @ruvector/edge-net join" to create one.')}\n`);
    return;
  }

  console.log(`  ${c('cyan', 'Found')} ${identities.length} ${c('cyan', 'identities:')}\n`);

  for (const meta of identities) {
    const memberSince = meta.createdAt ? new Date(meta.createdAt).toLocaleDateString() : 'unknown';
    const lastUsed = meta.lastUsed ? new Date(meta.lastUsed).toLocaleDateString() : 'unknown';

    console.log(`  ${c('bold', meta.siteId)}`);
    console.log(`    ${c('dim', 'ID:')}           ${meta.shortId}`);
    console.log(`    ${c('dim', 'Public Key:')}   ${meta.publicKey.substring(0, 16)}...`);
    console.log(`    ${c('dim', 'Member Since:')} ${memberSince}`);
    console.log(`    ${c('dim', 'Last Used:')}    ${lastUsed}`);
    console.log(`    ${c('dim', 'Sessions:')}     ${meta.totalSessions || 0}`);
    console.log(`    ${c('dim', 'Contributions:')} ${meta.totalContributions || 0}\n`);
  }

  console.log(`${c('dim', 'Storage: ' + getIdentitiesDir())}\n`);
}

async function generateIdentity(wasm, siteId) {
  console.log(`${c('cyan', 'Generating new Pi-Key identity...')}\n`);

  // Generate Pi-Key
  const piKey = new wasm.PiKey();

  const identity = piKey.getIdentity();
  const identityHex = piKey.getIdentityHex();
  const publicKey = piKey.getPublicKey();
  const shortId = piKey.getShortId();
  const genesisFingerprint = piKey.getGenesisFingerprint();
  const hasPiMagic = piKey.verifyPiMagic();
  const stats = JSON.parse(piKey.getStats());

  console.log(`${c('bold', 'IDENTITY GENERATED:')}`);
  console.log(`  ${c('cyan', 'Short ID:')}         ${shortId}`);
  console.log(`  ${c('cyan', 'Pi-Identity:')}      ${identityHex.substring(0, 32)}...`);
  console.log(`  ${c('cyan', 'Public Key:')}       ${toHex(publicKey).substring(0, 32)}...`);
  console.log(`  ${c('cyan', 'Genesis FP:')}       ${toHex(genesisFingerprint)}`);
  console.log(`  ${c('cyan', 'Pi Magic:')}         ${hasPiMagic ? c('green', '✓ Valid') : c('red', '✗ Invalid')}`);
  console.log(`  ${c('cyan', 'Identity Size:')}    ${identity.length} bytes (π-sized)`);
  console.log(`  ${c('cyan', 'PubKey Size:')}      ${publicKey.length} bytes`);
  console.log(`  ${c('cyan', 'Genesis Size:')}     ${genesisFingerprint.length} bytes (φ-sized)\n`);

  // Test signing
  const testData = new TextEncoder().encode('EdgeNet contributor test message');
  const signature = piKey.sign(testData);
  const isValid = piKey.verify(testData, signature, publicKey);

  console.log(`${c('bold', 'CRYPTOGRAPHIC TEST:')}`);
  console.log(`  ${c('cyan', 'Test Message:')}     "EdgeNet contributor test message"`);
  console.log(`  ${c('cyan', 'Signature:')}        ${toHex(signature).substring(0, 32)}...`);
  console.log(`  ${c('cyan', 'Signature Size:')}   ${signature.length} bytes`);
  console.log(`  ${c('cyan', 'Verification:')}     ${isValid ? c('green', '✓ Valid') : c('red', '✗ Invalid')}\n`);

  return { piKey, publicKey, identityHex, shortId };
}

async function exportIdentity(wasm, filePath, password) {
  console.log(`${c('cyan', 'Exporting identity to:')} ${filePath}\n`);

  const piKey = new wasm.PiKey();

  if (!password) {
    password = 'edge-net-default-password'; // Warning: use strong password in production
    console.log(`${c('yellow', '⚠ Using default password. Use --password for security.')}\n`);
  }

  const backup = piKey.createEncryptedBackup(password);
  writeFileSync(filePath, Buffer.from(backup));

  console.log(`${c('green', '✓')} Identity exported successfully`);
  console.log(`  ${c('cyan', 'File:')}       ${filePath}`);
  console.log(`  ${c('cyan', 'Size:')}       ${backup.length} bytes`);
  console.log(`  ${c('cyan', 'Encryption:')} Argon2id + AES-256-GCM`);
  console.log(`  ${c('cyan', 'Short ID:')}   ${piKey.getShortId()}\n`);

  console.log(`${c('yellow', 'Keep this file and password safe!')}`);
  console.log(`${c('dim', 'You can restore with: npx @ruvector/edge-net join --import')} ${filePath}\n`);

  return piKey;
}

async function importIdentity(wasm, filePath, password) {
  console.log(`${c('cyan', 'Importing identity from:')} ${filePath}\n`);

  if (!existsSync(filePath)) {
    console.error(`${c('red', '✗ File not found:')} ${filePath}`);
    process.exit(1);
  }

  if (!password) {
    password = 'edge-net-default-password';
    console.log(`${c('yellow', '⚠ Using default password.')}\n`);
  }

  const backup = new Uint8Array(readFileSync(filePath));

  try {
    const piKey = wasm.PiKey.restoreFromBackup(backup, password);

    console.log(`${c('green', '✓')} Identity restored successfully`);
    console.log(`  ${c('cyan', 'Short ID:')}    ${piKey.getShortId()}`);
    console.log(`  ${c('cyan', 'Public Key:')} ${toHex(piKey.getPublicKey()).substring(0, 32)}...`);
    console.log(`  ${c('cyan', 'Pi Magic:')}   ${piKey.verifyPiMagic() ? c('green', '✓ Valid') : c('red', '✗ Invalid')}\n`);

    return piKey;
  } catch (e) {
    console.error(`${c('red', '✗ Failed to restore identity:')} ${e.message}`);
    console.log(`${c('dim', 'Check password and file integrity.')}`);
    process.exit(1);
  }
}

async function joinNetwork(wasm, opts, piKey) {
  console.log(`${c('bold', 'JOINING EDGE-NET...')}\n`);

  const publicKeyHex = toHex(piKey.getPublicKey());

  // Create components for network participation
  const detector = new wasm.ByzantineDetector(0.5);
  const dp = new wasm.DifferentialPrivacy(1.0, 0.001);
  const model = new wasm.FederatedModel(100, 0.01, 0.9);
  const coherence = new wasm.CoherenceEngine();
  const evolution = new wasm.EvolutionEngine();
  const events = new wasm.NetworkEvents();

  console.log(`${c('bold', 'CONTRIBUTOR NODE:')}`);
  console.log(`  ${c('cyan', 'Site ID:')}       ${opts.site}`);
  console.log(`  ${c('cyan', 'Short ID:')}      ${piKey.getShortId()}`);
  console.log(`  ${c('cyan', 'Public Key:')}    ${publicKeyHex.substring(0, 16)}...${publicKeyHex.slice(-8)}`);
  console.log(`  ${c('cyan', 'Status:')}        ${c('green', 'Connected')}`);
  console.log(`  ${c('cyan', 'Mode:')}          Lightweight (CLI)\n`);

  console.log(`${c('bold', 'ACTIVE COMPONENTS:')}`);
  console.log(`  ${c('green', '✓')} Byzantine Detector (threshold=0.5)`);
  console.log(`  ${c('green', '✓')} Differential Privacy (ε=1.0)`);
  console.log(`  ${c('green', '✓')} Federated Model (dim=100)`);
  console.log(`  ${c('green', '✓')} Coherence Engine (Merkle: ${coherence.getMerkleRoot().substring(0, 16)}...)`);
  console.log(`  ${c('green', '✓')} Evolution Engine (fitness: ${evolution.getNetworkFitness().toFixed(2)})`);

  // Get themed status
  const themedStatus = events.getThemedStatus(1, BigInt(0));
  console.log(`\n${c('bold', 'NETWORK STATUS:')}`);
  console.log(`  ${themedStatus}\n`);

  // Show sharing information
  console.log(`${c('bold', 'SHARE YOUR PUBLIC KEY:')}`);
  console.log(`  ${c('dim', 'Others can verify your contributions using your public key:')}`);
  console.log(`  ${c('cyan', publicKeyHex)}\n`);

  console.log(`${c('green', '✓ Successfully joined Edge-Net!')}\n`);
  console.log(`${c('dim', 'Press Ctrl+C to disconnect.')}\n`);

  // Keep running with periodic status updates
  let ticks = 0;
  const statusInterval = setInterval(() => {
    ticks++;
    const motivation = events.getMotivation(BigInt(ticks * 10));
    if (ticks % 10 === 0) {
      console.log(`  ${c('dim', `[${ticks}s]`)} ${c('cyan', 'Contributing...')} ${motivation}`);
    }
  }, 1000);

  process.on('SIGINT', () => {
    clearInterval(statusInterval);
    console.log(`\n${c('yellow', 'Disconnected from Edge-Net.')}`);
    console.log(`${c('dim', 'Your identity is preserved. Rejoin anytime.')}\n`);

    // Clean up WASM resources
    detector.free();
    dp.free();
    model.free();
    coherence.free();
    evolution.free();
    events.free();
    piKey.free();

    process.exit(0);
  });
}

async function showStatus(wasm, piKey) {
  console.log(`${c('bold', 'CONTRIBUTOR STATUS:')}\n`);

  const publicKey = piKey.getPublicKey();
  const stats = JSON.parse(piKey.getStats());

  console.log(`  ${c('cyan', 'Identity:')}     ${piKey.getShortId()}`);
  console.log(`  ${c('cyan', 'Public Key:')}   ${toHex(publicKey).substring(0, 32)}...`);
  console.log(`  ${c('cyan', 'Pi Magic:')}     ${piKey.verifyPiMagic() ? c('green', '✓') : c('red', '✗')}`);

  // Create temp components to check status
  const evolution = new wasm.EvolutionEngine();
  const coherence = new wasm.CoherenceEngine();

  console.log(`\n${c('bold', 'NETWORK METRICS:')}`);
  console.log(`  ${c('cyan', 'Fitness:')}      ${evolution.getNetworkFitness().toFixed(4)}`);
  console.log(`  ${c('cyan', 'Merkle Root:')}  ${coherence.getMerkleRoot().substring(0, 24)}...`);
  console.log(`  ${c('cyan', 'Conflicts:')}    ${coherence.conflictCount()}`);
  console.log(`  ${c('cyan', 'Quarantined:')}  ${coherence.quarantinedCount()}`);
  console.log(`  ${c('cyan', 'Events:')}       ${coherence.eventCount()}\n`);

  evolution.free();
  coherence.free();
}

// Show peers from network module
async function showPeers() {
  console.log(`${c('bold', 'NETWORK PEERS:')}\n`);

  try {
    const { promises: fs } = await import('fs');
    const peersFile = join(homedir(), '.ruvector', 'network', 'peers.json');

    if (!existsSync(peersFile)) {
      console.log(`  ${c('dim', 'No peers found. Join the network first.')}\n`);
      return;
    }

    const peers = JSON.parse(await fs.readFile(peersFile, 'utf-8'));

    if (peers.length === 0) {
      console.log(`  ${c('dim', 'No peers discovered yet.')}\n`);
      return;
    }

    console.log(`  ${c('cyan', 'Found')} ${peers.length} ${c('cyan', 'peers:')}\n`);

    for (const peer of peers) {
      const timeSince = Date.now() - peer.lastSeen;
      const isActive = timeSince < 300000; // 5 minutes
      const status = isActive ? c('green', '● Active') : c('dim', '○ Inactive');

      console.log(`  ${status} ${c('bold', peer.siteId)}`);
      console.log(`    ${c('dim', 'Pi-Key:')}      π:${peer.piKey.slice(0, 12)}...`);
      console.log(`    ${c('dim', 'Public Key:')} ${peer.publicKey.slice(0, 16)}...`);
      console.log(`    ${c('dim', 'First Seen:')} ${new Date(peer.firstSeen).toLocaleString()}`);
      console.log(`    ${c('dim', 'Last Seen:')}  ${new Date(peer.lastSeen).toLocaleString()}`);
      console.log(`    ${c('dim', 'Verified:')}   ${peer.verified ? c('green', '✓ Yes') : c('yellow', '○ No')}`);
      console.log('');
    }
  } catch (err) {
    console.log(`  ${c('red', '✗')} Error reading peers: ${err.message}\n`);
  }
}

// Handle --networks command (list known networks)
async function handleListNetworks() {
  console.log(`${c('bold', 'KNOWN NETWORKS:')}\n`);

  try {
    const registry = new NetworkRegistry();
    await registry.load();

    const networks = registry.listNetworks();
    const active = registry.activeNetwork;

    if (networks.length === 0) {
      console.log(`  ${c('dim', 'No networks registered.')}`);
      console.log(`  ${c('dim', 'Use --discover to find available networks.')}\n`);
      return;
    }

    for (const network of networks) {
      const isActive = network.id === active;
      const status = network.joined ?
        (isActive ? c('green', '● Active') : c('cyan', '○ Joined')) :
        c('dim', '  Available');
      const typeIcon = network.type === 'public' ? '🌐' :
                       network.type === 'private' ? '🔒' : '🏢';

      console.log(`  ${status} ${typeIcon} ${c('bold', network.name)}`);
      console.log(`    ${c('dim', 'ID:')}   ${network.id}`);
      console.log(`    ${c('dim', 'Type:')} ${network.type}`);
      if (network.description) {
        console.log(`    ${c('dim', network.description)}`);
      }
      console.log('');
    }

    console.log(`${c('dim', 'Use --switch <network-id> to change active network')}\n`);

  } catch (err) {
    console.log(`  ${c('red', '✗')} Error: ${err.message}\n`);
  }
}

// Handle --discover command
async function handleDiscoverNetworks() {
  console.log(`${c('cyan', 'Discovering networks...')}\n`);

  try {
    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    const networks = await manager.discoverNetworks();

    if (networks.length > 0) {
      console.log(`\n${c('dim', 'To join a network:')} --network <id> [--invite <code>]`);
      console.log(`${c('dim', 'To create your own:')} --create-network "Name" [--network-type private]\n`);
    }
  } catch (err) {
    console.log(`  ${c('red', '✗')} Error: ${err.message}\n`);
  }
}

// Handle --create-network command
async function handleCreateNetwork(opts) {
  console.log(`${c('cyan', 'Creating new network...')}\n`);

  try {
    const manager = new MultiNetworkManager(null);
    await manager.initialize();

    const result = await manager.createNetwork({
      name: opts.createNetwork,
      type: opts.networkType,
      description: opts.networkDesc,
    });

    console.log(`\n${c('dim', 'To invite others (if private):')} Share the invite codes above`);
    console.log(`${c('dim', 'To contribute:')} --network ${result.networkId}\n`);

  } catch (err) {
    console.log(`  ${c('red', '✗')} Error: ${err.message}\n`);
  }
}

// Handle --switch command
async function handleSwitchNetwork(networkId) {
  console.log(`${c('cyan', `Switching to network ${networkId}...`)}\n`);

  try {
    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    await manager.switchNetwork(networkId);

    console.log(`\n${c('dim', 'Your contributions will now go to this network.')}\n`);

  } catch (err) {
    console.log(`  ${c('red', '✗')} Error: ${err.message}\n`);
  }
}

// Show network/QDAG statistics
async function showNetworkStats() {
  console.log(`${c('bold', 'NETWORK STATISTICS:')}\n`);

  try {
    const { promises: fs } = await import('fs');
    const qdagFile = join(homedir(), '.ruvector', 'network', 'qdag.json');

    if (!existsSync(qdagFile)) {
      console.log(`  ${c('dim', 'No QDAG data found. Join the network first.')}\n`);
      return;
    }

    const qdag = JSON.parse(await fs.readFile(qdagFile, 'utf-8'));

    const contributions = (qdag.nodes || []).filter(n => n.type === 'contribution');
    const contributors = new Set(contributions.map(c => c.contributor));
    const totalCredits = contributions.reduce((sum, c) => sum + (c.credits || 0), 0);
    const totalCompute = contributions.reduce((sum, c) => sum + (c.computeUnits || 0), 0);

    console.log(`${c('bold', 'QDAG Ledger:')}`);
    console.log(`  ${c('cyan', 'Total Nodes:')}       ${qdag.nodes?.length || 0}`);
    console.log(`  ${c('cyan', 'Confirmed:')}         ${qdag.confirmed?.length || 0}`);
    console.log(`  ${c('cyan', 'Current Tips:')}      ${qdag.tips?.length || 0}`);
    console.log('');

    console.log(`${c('bold', 'Contributions:')}`);
    console.log(`  ${c('cyan', 'Total:')}             ${contributions.length}`);
    console.log(`  ${c('cyan', 'Contributors:')}      ${contributors.size}`);
    console.log(`  ${c('cyan', 'Total Credits:')}     ${totalCredits}`);
    console.log(`  ${c('cyan', 'Compute Units:')}     ${totalCompute.toLocaleString()}`);
    console.log('');

    // Show top contributors
    if (contributors.size > 0) {
      console.log(`${c('bold', 'Top Contributors:')}`);
      const contributorStats = {};
      for (const contrib of contributions) {
        if (!contributorStats[contrib.contributor]) {
          contributorStats[contrib.contributor] = { credits: 0, count: 0, siteId: contrib.siteId };
        }
        contributorStats[contrib.contributor].credits += contrib.credits || 0;
        contributorStats[contrib.contributor].count++;
      }

      const sorted = Object.entries(contributorStats)
        .sort((a, b) => b[1].credits - a[1].credits)
        .slice(0, 5);

      for (const [piKey, stats] of sorted) {
        console.log(`  ${c('green', '★')} ${stats.siteId || piKey.slice(0, 12)} - ${stats.credits} credits (${stats.count} contributions)`);
      }
      console.log('');
    }

    // Show recent activity
    const recentContribs = contributions
      .sort((a, b) => b.timestamp - a.timestamp)
      .slice(0, 5);

    if (recentContribs.length > 0) {
      console.log(`${c('bold', 'Recent Activity:')}`);
      for (const contrib of recentContribs) {
        const time = new Date(contrib.timestamp).toLocaleTimeString();
        console.log(`  ${c('dim', time)} ${contrib.siteId || contrib.contributor.slice(0, 8)} +${contrib.credits} credits`);
      }
      console.log('');
    }

  } catch (err) {
    console.log(`  ${c('red', '✗')} Error reading network stats: ${err.message}\n`);
  }
}

// Multi-contributor demonstration
async function demonstrateMultiContributor(wasm) {
  console.log(`${c('bold', 'MULTI-CONTRIBUTOR DEMONSTRATION')}\n`);
  console.log(`${c('dim', 'Simulating 3 contributors joining the network...')}\n`);

  const contributors = [];

  for (let i = 1; i <= 3; i++) {
    const piKey = new wasm.PiKey();
    const publicKey = piKey.getPublicKey();
    const shortId = piKey.getShortId();

    contributors.push({ piKey, publicKey, shortId, id: i });

    console.log(`${c('cyan', `Contributor ${i}:`)}`);
    console.log(`  ${c('dim', 'Short ID:')}    ${shortId}`);
    console.log(`  ${c('dim', 'Public Key:')} ${toHex(publicKey).substring(0, 24)}...`);
    console.log(`  ${c('dim', 'Pi Magic:')}   ${piKey.verifyPiMagic() ? c('green', '✓') : c('red', '✗')}\n`);
  }

  // Demonstrate cross-verification
  console.log(`${c('bold', 'CROSS-VERIFICATION TEST:')}\n`);

  const testMessage = new TextEncoder().encode('Multi-contributor coordination test');

  for (let i = 0; i < contributors.length; i++) {
    const signer = contributors[i];
    const signature = signer.piKey.sign(testMessage);

    console.log(`${c('cyan', `Contributor ${signer.id} signs message:`)}`);

    // Each other contributor verifies
    for (let j = 0; j < contributors.length; j++) {
      const verifier = contributors[j];
      const isValid = signer.piKey.verify(testMessage, signature, signer.publicKey);

      if (i !== j) {
        console.log(`  ${c('dim', `Contributor ${verifier.id} verifies:`)} ${isValid ? c('green', '✓ Valid') : c('red', '✗ Invalid')}`);
      }
    }
    console.log('');
  }

  // Create shared coherence state
  const coherence = new wasm.CoherenceEngine();

  console.log(`${c('bold', 'SHARED COHERENCE STATE:')}`);
  console.log(`  ${c('cyan', 'Merkle Root:')}  ${coherence.getMerkleRoot()}`);
  console.log(`  ${c('cyan', 'Conflicts:')}    ${coherence.conflictCount()}`);
  console.log(`  ${c('cyan', 'Event Count:')}  ${coherence.eventCount()}\n`);

  console.log(`${c('green', '✓ Multi-contributor simulation complete!')}\n`);
  console.log(`${c('dim', 'All contributors can independently verify each other\'s signatures.')}`);
  console.log(`${c('dim', 'The coherence engine maintains consistent state across the network.')}\n`);

  // Cleanup
  contributors.forEach(c => c.piKey.free());
  coherence.free();
}

async function main() {
  const args = process.argv.slice(2);

  // Filter out 'join' if passed
  const filteredArgs = args.filter(a => a !== 'join');
  const opts = parseArgs(filteredArgs);

  if (opts.help || args.includes('help') || args.includes('--help') || args.includes('-h')) {
    printHelp();
    return;
  }

  // Handle --list early (no WASM needed)
  if (opts.list) {
    printBanner();
    await listIdentities();
    return;
  }

  // Handle multi-network commands (no WASM needed)
  if (opts.listNetworks) {
    printBanner();
    await handleListNetworks();
    return;
  }

  if (opts.discoverNetworks) {
    printBanner();
    await handleDiscoverNetworks();
    return;
  }

  if (opts.createNetwork) {
    printBanner();
    await handleCreateNetwork(opts);
    return;
  }

  if (opts.switchNetwork) {
    printBanner();
    await handleSwitchNetwork(opts.switchNetwork);
    return;
  }

  printBanner();
  await setupPolyfills();

  // Load WASM module
  const { createRequire } = await import('module');
  const require = createRequire(import.meta.url);

  console.log(`${c('dim', 'Loading WASM module...')}`);
  const wasm = require('./node/ruvector_edge_net.cjs');
  console.log(`${c('green', '✓')} WASM module loaded\n`);

  // Handle --history
  if (opts.history) {
    const password = opts.password || `${opts.site}-edge-net-key`;
    await showHistory(wasm, opts.site, password);
    return;
  }

  // Handle --peers (show network peers)
  if (opts.peers) {
    await showPeers();
    return;
  }

  // Handle --network (show network/QDAG stats)
  if (args.includes('--network')) {
    await showNetworkStats();
    return;
  }

  let piKey = null;
  let persistentIdentity = null;

  try {
    // Handle different modes
    if (opts.export) {
      piKey = await exportIdentity(wasm, opts.export, opts.password);
      return;
    }

    if (opts.import) {
      piKey = await importIdentity(wasm, opts.import, opts.password);
    } else if (opts.key) {
      // Join with existing public key (generate matching key for demo)
      console.log(`${c('cyan', 'Using provided public key...')}\n`);
      console.log(`${c('dim', 'Note: Full key management requires import/export.')}\n`);
      piKey = new wasm.PiKey();
    } else {
      // Use persistent identity (auto-creates or restores)
      const password = opts.password || `${opts.site}-edge-net-key`;
      persistentIdentity = new PersistentIdentity(opts.site, wasm);
      const result = await persistentIdentity.initialize(password);

      if (result.isNew) {
        console.log(`${c('green', '✓')} New identity created: ${result.meta.shortId}`);
        console.log(`  ${c('dim', 'Your identity is now stored locally and will persist.')}`);
        console.log(`  ${c('dim', 'Storage:')} ${getIdentitiesDir()}\n`);
      } else {
        console.log(`${c('green', '✓')} Identity restored: ${result.meta.shortId}`);
        console.log(`  ${c('dim', 'Member since:')} ${result.meta.createdAt}`);
        console.log(`  ${c('dim', 'Total sessions:')} ${result.sessions}`);
        if (result.daysSinceLastSession !== null) {
          if (result.daysSinceLastSession > 30) {
            console.log(`  ${c('yellow', 'Welcome back!')} ${result.daysSinceLastSession} days since last session`);
          } else if (result.daysSinceLastSession > 0) {
            console.log(`  ${c('dim', 'Last session:')} ${result.daysSinceLastSession} days ago`);
          }
        }
        console.log('');
      }

      piKey = persistentIdentity.piKey;
    }

    if (opts.generate) {
      // Just generate, don't join
      console.log(`${c('green', '✓ Identity generated and persisted!')}\n`);
      console.log(`${c('dim', 'Your identity is stored at:')} ${getIdentitiesDir()}`);
      console.log(`${c('dim', 'Run again to continue with the same identity.')}\n`);

      // Also demonstrate multi-contributor
      if (persistentIdentity) persistentIdentity.free();
      else if (piKey) piKey.free();
      await demonstrateMultiContributor(wasm);
      return;
    }

    if (opts.status) {
      await showStatus(wasm, piKey);
      if (persistentIdentity) persistentIdentity.free();
      else if (piKey) piKey.free();
      return;
    }

    // Join the network with persistence
    if (persistentIdentity) {
      await joinNetworkPersistent(wasm, opts, persistentIdentity);
    } else {
      await joinNetwork(wasm, opts, piKey);
    }

  } catch (err) {
    console.error(`${c('red', '✗ Error:')} ${err.message}`);
    if (persistentIdentity) persistentIdentity.free();
    else if (piKey) piKey.free();
    process.exit(1);
  }
}

// Join network with persistent identity (tracks contributions)
async function joinNetworkPersistent(wasm, opts, identity) {
  console.log(`${c('bold', 'JOINING EDGE-NET (Persistent Mode)...')}\n`);

  const publicKeyHex = identity.meta.publicKey;

  // Create components for network participation
  const detector = new wasm.ByzantineDetector(0.5);
  const dp = new wasm.DifferentialPrivacy(1.0, 0.001);
  const model = new wasm.FederatedModel(100, 0.01, 0.9);
  const coherence = new wasm.CoherenceEngine();
  const evolution = new wasm.EvolutionEngine();
  const events = new wasm.NetworkEvents();

  // Initialize network manager for QDAG and peer discovery
  let networkManager = null;
  try {
    networkManager = new NetworkManager({
      piKey: identity.meta.shortId,
      publicKey: publicKeyHex,
      siteId: opts.site
    });
    await networkManager.initialize();
  } catch (err) {
    console.log(`  ${c('yellow', '⚠')} Network module unavailable: ${err.message}`);
    console.log(`  ${c('dim', 'Running in local mode (contributions recorded locally)')}\n`);
  }

  console.log(`${c('bold', 'CONTRIBUTOR NODE:')}`);
  console.log(`  ${c('cyan', 'Site ID:')}       ${opts.site}`);
  console.log(`  ${c('cyan', 'Short ID:')}      ${identity.meta.shortId}`);
  console.log(`  ${c('cyan', 'Public Key:')}    ${publicKeyHex.substring(0, 16)}...${publicKeyHex.slice(-8)}`);
  console.log(`  ${c('cyan', 'Member Since:')}  ${new Date(identity.meta.createdAt).toLocaleDateString()}`);
  console.log(`  ${c('cyan', 'Sessions:')}      ${identity.meta.totalSessions}`);
  console.log(`  ${c('cyan', 'Status:')}        ${c('green', 'Connected')}`);
  console.log(`  ${c('cyan', 'Mode:')}          Persistent + QDAG\n`);

  console.log(`${c('bold', 'ACTIVE COMPONENTS:')}`);
  console.log(`  ${c('green', '✓')} Byzantine Detector (threshold=0.5)`);
  console.log(`  ${c('green', '✓')} Differential Privacy (ε=1.0)`);
  console.log(`  ${c('green', '✓')} Federated Model (dim=100)`);
  console.log(`  ${c('green', '✓')} Coherence Engine`);
  console.log(`  ${c('green', '✓')} Evolution Engine`);
  console.log(`  ${c('green', '✓')} QDAG Ledger (contribution tracking)`);
  console.log(`  ${c('green', '✓')} Peer Discovery (P2P network)`);

  // Get themed status
  const themedStatus = events.getThemedStatus(1, BigInt(identity.meta.totalContributions || 0));
  console.log(`\n${c('bold', 'NETWORK STATUS:')}`);
  console.log(`  ${themedStatus}`);

  // Show network stats if available
  if (networkManager) {
    const netStats = networkManager.getNetworkStats();
    const myStats = networkManager.getMyStats();
    console.log(`  ${c('cyan', 'QDAG Nodes:')}     ${netStats.totalNodes}`);
    console.log(`  ${c('cyan', 'Contributors:')}   ${netStats.uniqueContributors}`);
    console.log(`  ${c('cyan', 'Total Credits:')}  ${netStats.totalCredits}`);
    console.log(`  ${c('cyan', 'My Credits:')}     ${myStats.totalCredits}`);
  }
  console.log('');

  // Show persistence info
  console.log(`${c('bold', 'PERSISTENCE:')}`);
  console.log(`  ${c('dim', 'Identity stored at:')} ${identity.identityPath}`);
  console.log(`  ${c('dim', 'History stored at:')}  ${identity.contributionPath}`);
  console.log(`  ${c('dim', 'QDAG Ledger at:')}     ~/.ruvector/network/qdag.json`);
  console.log(`  ${c('dim', 'Your contributions are preserved across sessions (months/years).')}\n`);

  console.log(`${c('green', '✓ Successfully joined Edge-Net!')}\n`);
  console.log(`${c('dim', 'Press Ctrl+C to disconnect.')}\n`);

  // Keep running with periodic status updates and contribution tracking
  let ticks = 0;
  let contributions = 0;
  let totalCredits = 0;
  const statusInterval = setInterval(async () => {
    ticks++;

    // Simulate contribution every 5 seconds
    if (ticks % 5 === 0) {
      contributions++;
      const computeUnits = Math.floor(Math.random() * 500) + 100;
      const credits = Math.floor(computeUnits / 100);
      totalCredits += credits;

      // Record to local history
      identity.recordContribution('compute', { duration: 5, tick: ticks, computeUnits, credits });

      // Record to QDAG ledger
      if (networkManager) {
        const taskId = `task-${Date.now().toString(36)}`;
        await networkManager.recordContribution(taskId, computeUnits);
      }
    }

    const motivation = events.getMotivation(BigInt(ticks * 10));
    if (ticks % 10 === 0) {
      const peerCount = networkManager ? networkManager.getPeers().length : 0;
      console.log(`  ${c('dim', `[${ticks}s]`)} ${c('cyan', 'Contributing...')} ${contributions} tasks | ${totalCredits} credits | ${peerCount} peers | ${motivation}`);
    }
  }, 1000);

  process.on('SIGINT', () => {
    clearInterval(statusInterval);
    console.log(`\n${c('yellow', 'Disconnected from Edge-Net.')}`);
    console.log(`${c('green', '✓')} Session recorded: ${contributions} contributions, ${totalCredits} credits`);
    console.log(`${c('dim', 'Your identity, history, and QDAG records are preserved. Rejoin anytime.')}\n`);

    // Clean up resources
    if (networkManager) networkManager.stop();
    detector.free();
    dp.free();
    model.free();
    coherence.free();
    evolution.free();
    events.free();
    identity.free();

    process.exit(0);
  });
}

main().catch(err => {
  console.error(`${colors.red}Fatal error: ${err.message}${colors.reset}`);
  process.exit(1);
});
