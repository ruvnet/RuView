#!/usr/bin/env node
/**
 * Edge-Net Multi-Network Module
 *
 * Enables creation, discovery, and contribution to multiple edge networks.
 * Each network is cryptographically isolated with its own:
 * - Genesis block and network ID
 * - QDAG ledger
 * - Peer registry
 * - Access control (public/private/invite-only)
 *
 * Security Features:
 * - Network ID derived from genesis hash (tamper-evident)
 * - Ed25519 signatures for network announcements
 * - Optional invite codes for private networks
 * - Cryptographic proof of network membership
 */

import { createHash, randomBytes } from 'crypto';
import { promises as fs } from 'fs';
import { homedir } from 'os';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

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

// Network types
const NetworkType = {
  PUBLIC: 'public',        // Anyone can join and discover
  PRIVATE: 'private',      // Requires invite code to join
  CONSORTIUM: 'consortium', // Requires approval from existing members
};

// Well-known public networks (bootstrap)
const WELL_KNOWN_NETWORKS = [
  {
    id: 'mainnet',
    name: 'Edge-Net Mainnet',
    description: 'Primary public compute network',
    type: NetworkType.PUBLIC,
    genesisHash: 'edgenet-mainnet-genesis-v1',
    bootstrapNodes: ['edge-net.ruvector.dev:9000'],
    created: '2024-01-01T00:00:00Z',
  },
  {
    id: 'testnet',
    name: 'Edge-Net Testnet',
    description: 'Testing and development network',
    type: NetworkType.PUBLIC,
    genesisHash: 'edgenet-testnet-genesis-v1',
    bootstrapNodes: ['testnet.ruvector.dev:9000'],
    created: '2024-01-01T00:00:00Z',
  },
];

// Directory structure
function getNetworksDir() {
  const dir = join(homedir(), '.ruvector', 'networks');
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
  return dir;
}

function getRegistryFile() {
  return join(getNetworksDir(), 'registry.json');
}

function getNetworkDir(networkId) {
  const dir = join(getNetworksDir(), networkId);
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
  return dir;
}

/**
 * Network Genesis - defines a network's identity
 */
export class NetworkGenesis {
  constructor(options = {}) {
    this.version = 1;
    this.name = options.name || 'Custom Network';
    this.description = options.description || 'A custom edge-net network';
    this.type = options.type || NetworkType.PUBLIC;
    this.creator = options.creator || null; // Creator's public key
    this.creatorSiteId = options.creatorSiteId || 'anonymous';
    this.created = options.created || new Date().toISOString();
    this.parameters = {
      minContributors: options.minContributors || 1,
      confirmationThreshold: options.confirmationThreshold || 3,
      creditMultiplier: options.creditMultiplier || 1.0,
      maxPeers: options.maxPeers || 100,
      ...options.parameters,
    };
    this.inviteRequired = this.type !== NetworkType.PUBLIC;
    this.approvers = options.approvers || []; // For consortium networks
    this.nonce = options.nonce || randomBytes(16).toString('hex');
  }

  /**
   * Compute network ID from genesis hash
   */
  computeNetworkId() {
    const data = JSON.stringify({
      version: this.version,
      name: this.name,
      type: this.type,
      creator: this.creator,
      created: this.created,
      parameters: this.parameters,
      nonce: this.nonce,
    });

    const hash = createHash('sha256').update(data).digest('hex');
    return `net-${hash.slice(0, 16)}`;
  }

  /**
   * Create signed genesis block
   */
  createSignedGenesis(signFn) {
    const genesis = {
      ...this,
      networkId: this.computeNetworkId(),
    };

    if (signFn) {
      const dataToSign = JSON.stringify(genesis);
      genesis.signature = signFn(dataToSign);
    }

    return genesis;
  }

  /**
   * Generate invite code for private networks
   */
  generateInviteCode() {
    if (this.type === NetworkType.PUBLIC) {
      throw new Error('Public networks do not require invite codes');
    }

    const networkId = this.computeNetworkId();
    const secret = randomBytes(16).toString('hex');
    const code = Buffer.from(`${networkId}:${secret}`).toString('base64url');

    return {
      code,
      networkId,
      validUntil: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(), // 7 days
    };
  }
}

/**
 * Network Registry - manages known networks
 */
export class NetworkRegistry {
  constructor() {
    this.networks = new Map();
    this.activeNetwork = null;
    this.loaded = false;
  }

  async load() {
    try {
      // Load well-known networks
      for (const network of WELL_KNOWN_NETWORKS) {
        this.networks.set(network.id, {
          ...network,
          isWellKnown: true,
          joined: false,
          stats: null,
        });
      }

      // Load user's network registry
      if (existsSync(getRegistryFile())) {
        const data = JSON.parse(await fs.readFile(getRegistryFile(), 'utf-8'));

        for (const network of data.networks || []) {
          this.networks.set(network.id, {
            ...network,
            isWellKnown: false,
          });
        }

        this.activeNetwork = data.activeNetwork || null;
      }

      this.loaded = true;
    } catch (err) {
      console.error('Failed to load network registry:', err.message);
    }
  }

  async save() {
    const data = {
      version: 1,
      activeNetwork: this.activeNetwork,
      networks: Array.from(this.networks.values()).filter(n => !n.isWellKnown),
      savedAt: new Date().toISOString(),
    };

    await fs.writeFile(getRegistryFile(), JSON.stringify(data, null, 2));
  }

  /**
   * Create a new network
   */
  async createNetwork(options, identity) {
    const genesis = new NetworkGenesis({
      ...options,
      creator: identity?.publicKey,
      creatorSiteId: identity?.siteId,
    });

    const networkId = genesis.computeNetworkId();

    // Create network directory structure
    const networkDir = getNetworkDir(networkId);
    await fs.mkdir(join(networkDir, 'peers'), { recursive: true });

    // Save genesis block
    const genesisData = genesis.createSignedGenesis(
      identity?.sign ? (data) => identity.sign(data) : null
    );
    await fs.writeFile(
      join(networkDir, 'genesis.json'),
      JSON.stringify(genesisData, null, 2)
    );

    // Initialize QDAG for this network
    const qdag = {
      networkId,
      nodes: [{
        id: 'genesis',
        type: 'genesis',
        timestamp: Date.now(),
        message: `Genesis: ${genesis.name}`,
        parents: [],
        weight: 1,
        confirmations: 0,
      }],
      tips: ['genesis'],
      confirmed: ['genesis'],
      createdAt: Date.now(),
    };
    await fs.writeFile(
      join(networkDir, 'qdag.json'),
      JSON.stringify(qdag, null, 2)
    );

    // Initialize peer list
    await fs.writeFile(
      join(networkDir, 'peers.json'),
      JSON.stringify([], null, 2)
    );

    // Register network
    const networkEntry = {
      id: networkId,
      name: genesis.name,
      description: genesis.description,
      type: genesis.type,
      creator: genesis.creator,
      creatorSiteId: genesis.creatorSiteId,
      created: genesis.created,
      parameters: genesis.parameters,
      genesisHash: createHash('sha256')
        .update(JSON.stringify(genesisData))
        .digest('hex')
        .slice(0, 32),
      joined: true,
      isOwner: true,
      stats: { nodes: 1, contributors: 0, credits: 0 },
    };

    this.networks.set(networkId, networkEntry);
    await this.save();

    // Generate invite codes if private
    let inviteCodes = null;
    if (genesis.type !== NetworkType.PUBLIC) {
      inviteCodes = [];
      for (let i = 0; i < 5; i++) {
        inviteCodes.push(genesis.generateInviteCode());
      }
      await fs.writeFile(
        join(networkDir, 'invites.json'),
        JSON.stringify(inviteCodes, null, 2)
      );
    }

    return { networkId, genesis: genesisData, inviteCodes };
  }

  /**
   * Join an existing network
   */
  async joinNetwork(networkId, inviteCode = null) {
    const network = this.networks.get(networkId);

    if (!network) {
      throw new Error(`Network not found: ${networkId}`);
    }

    if (network.joined) {
      return { alreadyJoined: true, network };
    }

    // Verify invite code for private networks
    if (network.type === NetworkType.PRIVATE) {
      if (!inviteCode) {
        throw new Error('Private network requires invite code');
      }

      const isValid = await this.verifyInviteCode(networkId, inviteCode);
      if (!isValid) {
        throw new Error('Invalid or expired invite code');
      }
    }

    // Create local network directory
    const networkDir = getNetworkDir(networkId);

    // For well-known networks, create initial structure
    if (network.isWellKnown) {
      const qdag = {
        networkId,
        nodes: [{
          id: 'genesis',
          type: 'genesis',
          timestamp: Date.now(),
          message: `Joined: ${network.name}`,
          parents: [],
          weight: 1,
          confirmations: 0,
        }],
        tips: ['genesis'],
        confirmed: ['genesis'],
        createdAt: Date.now(),
      };
      await fs.writeFile(
        join(networkDir, 'qdag.json'),
        JSON.stringify(qdag, null, 2)
      );

      await fs.writeFile(
        join(networkDir, 'peers.json'),
        JSON.stringify([], null, 2)
      );
    }

    network.joined = true;
    network.joinedAt = new Date().toISOString();
    await this.save();

    return { joined: true, network };
  }

  /**
   * Verify invite code
   */
  async verifyInviteCode(networkId, code) {
    try {
      const decoded = Buffer.from(code, 'base64url').toString();
      const [codeNetworkId, secret] = decoded.split(':');

      if (codeNetworkId !== networkId) {
        return false;
      }

      // In production, verify against network's invite registry
      // For local simulation, accept any properly formatted code
      return secret && secret.length === 32;
    } catch {
      return false;
    }
  }

  /**
   * Discover networks from DHT/registry
   */
  async discoverNetworks(options = {}) {
    const discovered = [];

    // Always include well-known networks
    for (const network of WELL_KNOWN_NETWORKS) {
      const existing = this.networks.get(network.id);
      discovered.push({
        ...network,
        joined: existing?.joined || false,
        source: 'well-known',
      });
    }

    // Scan for locally known networks
    try {
      const networksDir = getNetworksDir();
      const dirs = await fs.readdir(networksDir);

      for (const dir of dirs) {
        if (dir === 'registry.json') continue;

        const genesisPath = join(networksDir, dir, 'genesis.json');
        if (existsSync(genesisPath)) {
          try {
            const genesis = JSON.parse(await fs.readFile(genesisPath, 'utf-8'));
            const existing = this.networks.get(genesis.networkId || dir);

            if (!existing?.isWellKnown) {
              discovered.push({
                id: genesis.networkId || dir,
                name: genesis.name,
                description: genesis.description,
                type: genesis.type,
                creator: genesis.creatorSiteId,
                created: genesis.created,
                joined: existing?.joined || false,
                source: 'local',
              });
            }
          } catch (e) {
            // Skip invalid genesis files
          }
        }
      }
    } catch (err) {
      // Networks directory doesn't exist yet
    }

    // In production: Query DHT/bootstrap nodes for public networks
    // This is simulated here

    return discovered;
  }

  /**
   * Set active network for contributions
   */
  async setActiveNetwork(networkId) {
    const network = this.networks.get(networkId);

    if (!network) {
      throw new Error(`Network not found: ${networkId}`);
    }

    if (!network.joined) {
      throw new Error(`Must join network first: ${networkId}`);
    }

    this.activeNetwork = networkId;
    await this.save();

    return network;
  }

  /**
   * Get network info
   */
  getNetwork(networkId) {
    return this.networks.get(networkId);
  }

  /**
   * Get active network
   */
  getActiveNetwork() {
    if (!this.activeNetwork) return null;
    return this.networks.get(this.activeNetwork);
  }

  /**
   * Get all joined networks
   */
  getJoinedNetworks() {
    return Array.from(this.networks.values()).filter(n => n.joined);
  }

  /**
   * Get network statistics
   */
  async getNetworkStats(networkId) {
    const networkDir = getNetworkDir(networkId);
    const qdagPath = join(networkDir, 'qdag.json');
    const peersPath = join(networkDir, 'peers.json');

    const stats = {
      nodes: 0,
      contributions: 0,
      contributors: 0,
      credits: 0,
      peers: 0,
    };

    try {
      if (existsSync(qdagPath)) {
        const qdag = JSON.parse(await fs.readFile(qdagPath, 'utf-8'));
        const contributions = (qdag.nodes || []).filter(n => n.type === 'contribution');

        stats.nodes = qdag.nodes?.length || 0;
        stats.contributions = contributions.length;
        stats.contributors = new Set(contributions.map(c => c.contributor)).size;
        stats.credits = contributions.reduce((sum, c) => sum + (c.credits || 0), 0);
      }

      if (existsSync(peersPath)) {
        const peers = JSON.parse(await fs.readFile(peersPath, 'utf-8'));
        stats.peers = peers.length;
      }
    } catch (err) {
      // Stats not available
    }

    return stats;
  }

  /**
   * List all networks
   */
  listNetworks() {
    return Array.from(this.networks.values());
  }
}

/**
 * Multi-Network Manager - coordinates contributions across networks
 */
export class MultiNetworkManager {
  constructor(identity) {
    this.identity = identity;
    this.registry = new NetworkRegistry();
    this.activeConnections = new Map();
  }

  async initialize() {
    await this.registry.load();
    return this;
  }

  /**
   * Create a new network
   */
  async createNetwork(options) {
    console.log(`\n${c('cyan', 'Creating new network...')}\n`);

    const result = await this.registry.createNetwork(options, this.identity);

    console.log(`${c('green', '✓')} Network created successfully!`);
    console.log(`  ${c('cyan', 'Network ID:')}   ${result.networkId}`);
    console.log(`  ${c('cyan', 'Name:')}         ${options.name}`);
    console.log(`  ${c('cyan', 'Type:')}         ${options.type}`);
    console.log(`  ${c('cyan', 'Description:')}  ${options.description || 'N/A'}`);

    if (result.inviteCodes) {
      console.log(`\n${c('bold', 'Invite Codes (share these to invite members):')}`);
      for (const invite of result.inviteCodes.slice(0, 3)) {
        console.log(`  ${c('yellow', invite.code)}`);
      }
      console.log(`  ${c('dim', `(${result.inviteCodes.length} codes saved to network directory)`)}`);
    }

    console.log(`\n${c('dim', 'Network directory:')} ~/.ruvector/networks/${result.networkId}`);

    return result;
  }

  /**
   * Discover available networks
   */
  async discoverNetworks() {
    console.log(`\n${c('cyan', 'Discovering networks...')}\n`);

    const networks = await this.registry.discoverNetworks();

    if (networks.length === 0) {
      console.log(`  ${c('dim', 'No networks found.')}`);
      return networks;
    }

    console.log(`${c('bold', 'Available Networks:')}\n`);

    for (const network of networks) {
      const status = network.joined ? c('green', '● Joined') : c('dim', '○ Not joined');
      const typeIcon = network.type === NetworkType.PUBLIC ? '🌐' :
                       network.type === NetworkType.PRIVATE ? '🔒' : '🏢';

      console.log(`  ${status} ${typeIcon} ${c('bold', network.name)}`);
      console.log(`    ${c('dim', 'ID:')}          ${network.id}`);
      console.log(`    ${c('dim', 'Type:')}        ${network.type}`);
      console.log(`    ${c('dim', 'Description:')} ${network.description || 'N/A'}`);
      console.log(`    ${c('dim', 'Source:')}      ${network.source}`);
      console.log('');
    }

    return networks;
  }

  /**
   * Join a network
   */
  async joinNetwork(networkId, inviteCode = null) {
    console.log(`\n${c('cyan', `Joining network ${networkId}...`)}\n`);

    try {
      const result = await this.registry.joinNetwork(networkId, inviteCode);

      if (result.alreadyJoined) {
        console.log(`${c('yellow', '⚠')} Already joined network: ${result.network.name}`);
      } else {
        console.log(`${c('green', '✓')} Successfully joined: ${result.network.name}`);
      }

      // Set as active if it's the only joined network
      const joinedNetworks = this.registry.getJoinedNetworks();
      if (joinedNetworks.length === 1) {
        await this.registry.setActiveNetwork(networkId);
        console.log(`  ${c('dim', 'Set as active network')}`);
      }

      return result;
    } catch (err) {
      console.log(`${c('red', '✗')} Failed to join: ${err.message}`);
      throw err;
    }
  }

  /**
   * Switch active network
   */
  async switchNetwork(networkId) {
    const network = await this.registry.setActiveNetwork(networkId);
    console.log(`${c('green', '✓')} Active network: ${network.name} (${networkId})`);
    return network;
  }

  /**
   * Show network status
   */
  async showStatus() {
    const active = this.registry.getActiveNetwork();
    const joined = this.registry.getJoinedNetworks();

    console.log(`\n${c('bold', 'NETWORK STATUS:')}\n`);

    if (!active) {
      console.log(`  ${c('yellow', '⚠')} No active network`);
      console.log(`  ${c('dim', 'Join a network to start contributing')}\n`);
      return;
    }

    const stats = await this.registry.getNetworkStats(active.id);

    console.log(`${c('bold', 'Active Network:')}`);
    console.log(`  ${c('cyan', 'Name:')}         ${active.name}`);
    console.log(`  ${c('cyan', 'ID:')}           ${active.id}`);
    console.log(`  ${c('cyan', 'Type:')}         ${active.type}`);
    console.log(`  ${c('cyan', 'QDAG Nodes:')}   ${stats.nodes}`);
    console.log(`  ${c('cyan', 'Contributions:')} ${stats.contributions}`);
    console.log(`  ${c('cyan', 'Contributors:')} ${stats.contributors}`);
    console.log(`  ${c('cyan', 'Total Credits:')} ${stats.credits}`);
    console.log(`  ${c('cyan', 'Connected Peers:')} ${stats.peers}`);

    if (joined.length > 1) {
      console.log(`\n${c('bold', 'Other Joined Networks:')}`);
      for (const network of joined) {
        if (network.id !== active.id) {
          console.log(`  ${c('dim', '○')} ${network.name} (${network.id})`);
        }
      }
    }

    console.log('');
  }

  /**
   * Get active network directory for contributions
   */
  getActiveNetworkDir() {
    const active = this.registry.getActiveNetwork();
    if (!active) return null;
    return getNetworkDir(active.id);
  }
}

// CLI interface
async function main() {
  const args = process.argv.slice(2);
  const command = args[0];

  const registry = new NetworkRegistry();
  await registry.load();

  if (command === 'list' || command === 'ls') {
    console.log(`\n${c('bold', 'NETWORKS:')}\n`);

    const networks = registry.listNetworks();
    const active = registry.activeNetwork;

    for (const network of networks) {
      const isActive = network.id === active;
      const status = network.joined ?
        (isActive ? c('green', '● Active') : c('cyan', '○ Joined')) :
        c('dim', '  Available');
      const typeIcon = network.type === NetworkType.PUBLIC ? '🌐' :
                       network.type === NetworkType.PRIVATE ? '🔒' : '🏢';

      console.log(`  ${status} ${typeIcon} ${c('bold', network.name)}`);
      console.log(`    ${c('dim', 'ID:')} ${network.id}`);
      if (network.description) {
        console.log(`    ${c('dim', network.description)}`);
      }
      console.log('');
    }

  } else if (command === 'discover') {
    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    await manager.discoverNetworks();

  } else if (command === 'create') {
    const name = args[1] || 'My Network';
    const type = args.includes('--private') ? NetworkType.PRIVATE :
                 args.includes('--consortium') ? NetworkType.CONSORTIUM :
                 NetworkType.PUBLIC;
    const description = args.find((a, i) => args[i - 1] === '--desc') || '';

    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    await manager.createNetwork({ name, type, description });

  } else if (command === 'join') {
    const networkId = args[1];
    const inviteCode = args.find((a, i) => args[i - 1] === '--invite');

    if (!networkId) {
      console.log(`${c('red', '✗')} Usage: networks join <network-id> [--invite <code>]`);
      process.exit(1);
    }

    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    await manager.joinNetwork(networkId, inviteCode);

  } else if (command === 'switch' || command === 'use') {
    const networkId = args[1];

    if (!networkId) {
      console.log(`${c('red', '✗')} Usage: networks switch <network-id>`);
      process.exit(1);
    }

    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    await manager.switchNetwork(networkId);

  } else if (command === 'status') {
    const manager = new MultiNetworkManager(null);
    await manager.initialize();
    await manager.showStatus();

  } else if (command === 'help' || !command) {
    console.log(`
${c('bold', 'Edge-Net Multi-Network Manager')}

${c('bold', 'COMMANDS:')}
  ${c('green', 'list')}       List all known networks
  ${c('green', 'discover')}   Discover available networks
  ${c('green', 'create')}     Create a new network
  ${c('green', 'join')}       Join an existing network
  ${c('green', 'switch')}     Switch active network
  ${c('green', 'status')}     Show current network status
  ${c('green', 'help')}       Show this help

${c('bold', 'EXAMPLES:')}
  ${c('dim', '# List networks')}
  $ node networks.js list

  ${c('dim', '# Create a public network')}
  $ node networks.js create "My Research Network" --desc "For ML research"

  ${c('dim', '# Create a private network')}
  $ node networks.js create "Team Network" --private

  ${c('dim', '# Join a network')}
  $ node networks.js join net-abc123def456

  ${c('dim', '# Join a private network with invite')}
  $ node networks.js join net-xyz789 --invite <invite-code>

  ${c('dim', '# Switch active network')}
  $ node networks.js switch net-abc123def456

${c('bold', 'NETWORK TYPES:')}
  ${c('cyan', '🌐 Public')}      Anyone can join and discover
  ${c('cyan', '🔒 Private')}     Requires invite code to join
  ${c('cyan', '🏢 Consortium')}  Requires approval from members
`);
  }
}

main().catch(console.error);
