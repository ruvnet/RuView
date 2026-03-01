#!/usr/bin/env node
/**
 * Edge-Net Network Module
 *
 * Handles:
 * - Bootstrap node discovery
 * - Peer announcement protocol
 * - QDAG contribution recording
 * - Contribution verification
 * - P2P message routing
 */

import { createHash, randomBytes } from 'crypto';
import { promises as fs } from 'fs';
import { homedir } from 'os';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Network configuration
const NETWORK_CONFIG = {
    // Bootstrap nodes (DHT entry points)
    bootstrapNodes: [
        { id: 'bootstrap-1', host: 'edge-net.ruvector.dev', port: 9000 },
        { id: 'bootstrap-2', host: 'edge-net-2.ruvector.dev', port: 9000 },
        { id: 'bootstrap-3', host: 'edge-net-3.ruvector.dev', port: 9000 },
    ],
    // Local network simulation for offline/testing
    localSimulation: true,
    // Peer discovery interval (ms)
    discoveryInterval: 30000,
    // Heartbeat interval (ms)
    heartbeatInterval: 10000,
    // Max peers per node
    maxPeers: 50,
    // QDAG sync interval (ms)
    qdagSyncInterval: 5000,
};

// Data directories
function getNetworkDir() {
    return join(homedir(), '.ruvector', 'network');
}

function getPeersFile() {
    return join(getNetworkDir(), 'peers.json');
}

function getQDAGFile() {
    return join(getNetworkDir(), 'qdag.json');
}

// Ensure directories exist
async function ensureDirectories() {
    await fs.mkdir(getNetworkDir(), { recursive: true });
}

/**
 * Peer Discovery and Management
 */
export class PeerManager {
    constructor(localIdentity) {
        this.localIdentity = localIdentity;
        this.peers = new Map();
        this.bootstrapNodes = NETWORK_CONFIG.bootstrapNodes;
        this.discoveryInterval = null;
        this.heartbeatInterval = null;
    }

    async initialize() {
        await ensureDirectories();
        await this.loadPeers();

        // Start discovery and heartbeat
        if (!NETWORK_CONFIG.localSimulation) {
            this.startDiscovery();
            this.startHeartbeat();
        }

        return this;
    }

    async loadPeers() {
        try {
            const data = await fs.readFile(getPeersFile(), 'utf-8');
            const peers = JSON.parse(data);
            for (const peer of peers) {
                this.peers.set(peer.piKey, peer);
            }
            console.log(`  📡 Loaded ${this.peers.size} known peers`);
        } catch (err) {
            // No peers file yet
            console.log('  📡 Starting fresh peer list');
        }
    }

    async savePeers() {
        const peers = Array.from(this.peers.values());
        await fs.writeFile(getPeersFile(), JSON.stringify(peers, null, 2));
    }

    /**
     * Announce this node to the network
     */
    async announce() {
        const announcement = {
            type: 'announce',
            piKey: this.localIdentity.piKey,
            publicKey: this.localIdentity.publicKey,
            siteId: this.localIdentity.siteId,
            timestamp: Date.now(),
            capabilities: ['compute', 'storage', 'verify'],
            version: '0.1.1',
        };

        // Sign the announcement
        announcement.signature = this.signMessage(JSON.stringify(announcement));

        // In local simulation, just record ourselves
        if (NETWORK_CONFIG.localSimulation) {
            await this.registerPeer({
                ...announcement,
                lastSeen: Date.now(),
                verified: true,
            });
            return announcement;
        }

        // In production, broadcast to bootstrap nodes
        for (const bootstrap of this.bootstrapNodes) {
            try {
                await this.sendToNode(bootstrap, announcement);
            } catch (err) {
                // Bootstrap node unreachable
            }
        }

        return announcement;
    }

    /**
     * Register a peer in the local peer table
     */
    async registerPeer(peer) {
        const existing = this.peers.get(peer.piKey);

        if (existing) {
            // Update last seen
            existing.lastSeen = Date.now();
            existing.verified = peer.verified || existing.verified;
        } else {
            // New peer
            this.peers.set(peer.piKey, {
                piKey: peer.piKey,
                publicKey: peer.publicKey,
                siteId: peer.siteId,
                capabilities: peer.capabilities || [],
                firstSeen: Date.now(),
                lastSeen: Date.now(),
                verified: peer.verified || false,
                contributions: 0,
            });
            console.log(`  🆕 New peer: ${peer.siteId} (π:${peer.piKey.slice(0, 8)})`);
        }

        await this.savePeers();
    }

    /**
     * Get active peers (seen in last 5 minutes)
     */
    getActivePeers() {
        const cutoff = Date.now() - 300000; // 5 minutes
        return Array.from(this.peers.values()).filter(p => p.lastSeen > cutoff);
    }

    /**
     * Get all known peers
     */
    getAllPeers() {
        return Array.from(this.peers.values());
    }

    /**
     * Verify a peer's identity
     */
    async verifyPeer(peer) {
        // Request identity proof
        const challenge = randomBytes(32).toString('hex');
        const response = await this.requestProof(peer, challenge);

        if (response && this.verifyProof(peer.publicKey, challenge, response)) {
            peer.verified = true;
            await this.savePeers();
            return true;
        }
        return false;
    }

    /**
     * Sign a message with local identity
     */
    signMessage(message) {
        // Simplified signing (in production uses Ed25519)
        const hash = createHash('sha256')
            .update(this.localIdentity.piKey)
            .update(message)
            .digest('hex');
        return hash;
    }

    /**
     * Verify a signature
     */
    verifySignature(publicKey, message, signature) {
        // Simplified verification
        return signature && signature.length === 64;
    }

    startDiscovery() {
        this.discoveryInterval = setInterval(async () => {
            await this.discoverPeers();
        }, NETWORK_CONFIG.discoveryInterval);
    }

    startHeartbeat() {
        this.heartbeatInterval = setInterval(async () => {
            await this.announce();
        }, NETWORK_CONFIG.heartbeatInterval);
    }

    async discoverPeers() {
        // Request peer lists from known peers
        for (const peer of this.getActivePeers()) {
            try {
                const newPeers = await this.requestPeerList(peer);
                for (const newPeer of newPeers) {
                    await this.registerPeer(newPeer);
                }
            } catch (err) {
                // Peer unreachable
            }
        }
    }

    // Placeholder network methods (implemented in production with WebRTC/WebSocket)
    async sendToNode(node, message) {
        // In production: WebSocket/WebRTC connection
        return { ok: true };
    }

    async requestProof(peer, challenge) {
        // In production: Request signed proof
        return this.signMessage(challenge);
    }

    verifyProof(publicKey, challenge, response) {
        return response && response.length > 0;
    }

    async requestPeerList(peer) {
        return [];
    }

    stop() {
        if (this.discoveryInterval) clearInterval(this.discoveryInterval);
        if (this.heartbeatInterval) clearInterval(this.heartbeatInterval);
    }
}

/**
 * QDAG (Quantum DAG) Contribution Ledger
 *
 * A directed acyclic graph that records all contributions
 * with cryptographic verification and consensus
 */
export class QDAGLedger {
    constructor(peerManager) {
        this.peerManager = peerManager;
        this.nodes = new Map();  // DAG nodes
        this.tips = new Set();   // Current tips (unconfirmed)
        this.confirmed = new Set();  // Confirmed nodes
        this.pendingContributions = [];
        this.syncInterval = null;
    }

    async initialize() {
        await this.loadLedger();

        if (!NETWORK_CONFIG.localSimulation) {
            this.startSync();
        }

        return this;
    }

    async loadLedger() {
        try {
            const data = await fs.readFile(getQDAGFile(), 'utf-8');
            const ledger = JSON.parse(data);

            for (const node of ledger.nodes || []) {
                this.nodes.set(node.id, node);
            }
            this.tips = new Set(ledger.tips || []);
            this.confirmed = new Set(ledger.confirmed || []);

            console.log(`  📊 Loaded QDAG: ${this.nodes.size} nodes, ${this.confirmed.size} confirmed`);
        } catch (err) {
            // Create genesis node
            const genesis = this.createNode({
                type: 'genesis',
                timestamp: Date.now(),
                message: 'Edge-Net QDAG Genesis',
            }, []);

            this.nodes.set(genesis.id, genesis);
            this.tips.add(genesis.id);
            this.confirmed.add(genesis.id);

            await this.saveLedger();
            console.log('  📊 Created QDAG genesis block');
        }
    }

    async saveLedger() {
        const ledger = {
            nodes: Array.from(this.nodes.values()),
            tips: Array.from(this.tips),
            confirmed: Array.from(this.confirmed),
            savedAt: Date.now(),
        };
        await fs.writeFile(getQDAGFile(), JSON.stringify(ledger, null, 2));
    }

    /**
     * Create a new QDAG node
     */
    createNode(data, parents) {
        const nodeData = {
            ...data,
            parents: parents,
            timestamp: Date.now(),
        };

        const id = createHash('sha256')
            .update(JSON.stringify(nodeData))
            .digest('hex')
            .slice(0, 16);

        return {
            id,
            ...nodeData,
            weight: 1,
            confirmations: 0,
        };
    }

    /**
     * Record a contribution to the QDAG
     */
    async recordContribution(contribution) {
        // Select parent tips (2 parents for DAG structure)
        const parents = this.selectTips(2);

        // Create contribution node
        const node = this.createNode({
            type: 'contribution',
            contributor: contribution.piKey,
            siteId: contribution.siteId,
            taskId: contribution.taskId,
            computeUnits: contribution.computeUnits,
            credits: contribution.credits,
            signature: contribution.signature,
        }, parents);

        // Add to DAG
        this.nodes.set(node.id, node);

        // Update tips
        for (const parent of parents) {
            this.tips.delete(parent);
        }
        this.tips.add(node.id);

        // Update parent weights (confirm path)
        await this.updateWeights(node.id);

        await this.saveLedger();

        console.log(`  📝 Recorded contribution ${node.id}: +${contribution.credits} credits`);

        return node;
    }

    /**
     * Select tips for new node parents
     */
    selectTips(count) {
        const tips = Array.from(this.tips);
        if (tips.length <= count) return tips;

        // Weighted random selection based on age
        const selected = [];
        const available = [...tips];

        while (selected.length < count && available.length > 0) {
            const idx = Math.floor(Math.random() * available.length);
            selected.push(available[idx]);
            available.splice(idx, 1);
        }

        return selected;
    }

    /**
     * Update weights along the path to genesis
     */
    async updateWeights(nodeId) {
        const visited = new Set();
        const queue = [nodeId];

        while (queue.length > 0) {
            const id = queue.shift();
            if (visited.has(id)) continue;
            visited.add(id);

            const node = this.nodes.get(id);
            if (!node) continue;

            node.weight = (node.weight || 0) + 1;
            node.confirmations = (node.confirmations || 0) + 1;

            // Check for confirmation threshold
            if (node.confirmations >= 3 && !this.confirmed.has(id)) {
                this.confirmed.add(id);
            }

            // Add parents to queue
            for (const parentId of node.parents || []) {
                queue.push(parentId);
            }
        }
    }

    /**
     * Get contribution stats for a contributor
     */
    getContributorStats(piKey) {
        const contributions = Array.from(this.nodes.values())
            .filter(n => n.type === 'contribution' && n.contributor === piKey);

        return {
            totalContributions: contributions.length,
            confirmedContributions: contributions.filter(c => this.confirmed.has(c.id)).length,
            totalCredits: contributions.reduce((sum, c) => sum + (c.credits || 0), 0),
            totalComputeUnits: contributions.reduce((sum, c) => sum + (c.computeUnits || 0), 0),
            firstContribution: contributions.length > 0
                ? Math.min(...contributions.map(c => c.timestamp))
                : null,
            lastContribution: contributions.length > 0
                ? Math.max(...contributions.map(c => c.timestamp))
                : null,
        };
    }

    /**
     * Get network-wide stats
     */
    getNetworkStats() {
        const contributions = Array.from(this.nodes.values())
            .filter(n => n.type === 'contribution');

        const contributors = new Set(contributions.map(c => c.contributor));

        return {
            totalNodes: this.nodes.size,
            totalContributions: contributions.length,
            confirmedNodes: this.confirmed.size,
            uniqueContributors: contributors.size,
            totalCredits: contributions.reduce((sum, c) => sum + (c.credits || 0), 0),
            totalComputeUnits: contributions.reduce((sum, c) => sum + (c.computeUnits || 0), 0),
            currentTips: this.tips.size,
        };
    }

    /**
     * Verify contribution integrity
     */
    async verifyContribution(nodeId) {
        const node = this.nodes.get(nodeId);
        if (!node) return { valid: false, reason: 'Node not found' };

        // Verify parents exist
        for (const parentId of node.parents || []) {
            if (!this.nodes.has(parentId)) {
                return { valid: false, reason: `Missing parent: ${parentId}` };
            }
        }

        // Verify signature (if peer available)
        const peer = this.peerManager.peers.get(node.contributor);
        if (peer && node.signature) {
            const dataToVerify = JSON.stringify({
                contributor: node.contributor,
                taskId: node.taskId,
                computeUnits: node.computeUnits,
                credits: node.credits,
            });

            if (!this.peerManager.verifySignature(peer.publicKey, dataToVerify, node.signature)) {
                return { valid: false, reason: 'Invalid signature' };
            }
        }

        return { valid: true, confirmations: node.confirmations };
    }

    /**
     * Sync QDAG with peers
     */
    startSync() {
        this.syncInterval = setInterval(async () => {
            await this.syncWithPeers();
        }, NETWORK_CONFIG.qdagSyncInterval);
    }

    async syncWithPeers() {
        const activePeers = this.peerManager.getActivePeers();

        for (const peer of activePeers.slice(0, 3)) {
            try {
                // Request missing nodes from peer
                const peerTips = await this.requestTips(peer);
                for (const tipId of peerTips) {
                    if (!this.nodes.has(tipId)) {
                        const node = await this.requestNode(peer, tipId);
                        if (node) {
                            await this.mergeNode(node);
                        }
                    }
                }
            } catch (err) {
                // Peer sync failed
            }
        }
    }

    async requestTips(peer) {
        // In production: Request tips via P2P
        return [];
    }

    async requestNode(peer, nodeId) {
        // In production: Request specific node via P2P
        return null;
    }

    async mergeNode(node) {
        if (this.nodes.has(node.id)) return;

        // Verify node before merging
        const verification = await this.verifyContribution(node.id);
        if (!verification.valid) return;

        this.nodes.set(node.id, node);
        await this.updateWeights(node.id);
        await this.saveLedger();
    }

    stop() {
        if (this.syncInterval) clearInterval(this.syncInterval);
    }
}

/**
 * Contribution Verifier
 *
 * Cross-verifies contributions between peers
 */
export class ContributionVerifier {
    constructor(peerManager, qdagLedger) {
        this.peerManager = peerManager;
        this.qdag = qdagLedger;
        this.verificationQueue = [];
    }

    /**
     * Submit contribution for verification
     */
    async submitContribution(contribution) {
        // Sign the contribution
        contribution.signature = this.peerManager.signMessage(
            JSON.stringify({
                contributor: contribution.piKey,
                taskId: contribution.taskId,
                computeUnits: contribution.computeUnits,
                credits: contribution.credits,
            })
        );

        // Record to local QDAG
        const node = await this.qdag.recordContribution(contribution);

        // In local simulation, self-verify
        if (NETWORK_CONFIG.localSimulation) {
            return {
                nodeId: node.id,
                verified: true,
                confirmations: 1,
            };
        }

        // In production, broadcast for peer verification
        const verifications = await this.broadcastForVerification(node);

        return {
            nodeId: node.id,
            verified: verifications.filter(v => v.valid).length >= 2,
            confirmations: verifications.length,
        };
    }

    /**
     * Broadcast contribution for peer verification
     */
    async broadcastForVerification(node) {
        const activePeers = this.peerManager.getActivePeers();
        const verifications = [];

        for (const peer of activePeers.slice(0, 5)) {
            try {
                const verification = await this.requestVerification(peer, node);
                verifications.push(verification);
            } catch (err) {
                // Peer verification failed
            }
        }

        return verifications;
    }

    async requestVerification(peer, node) {
        // In production: Request verification via P2P
        return { valid: true, peerId: peer.piKey };
    }

    /**
     * Verify a contribution from another peer
     */
    async verifyFromPeer(contribution, requestingPeer) {
        // Verify signature
        const valid = this.peerManager.verifySignature(
            requestingPeer.publicKey,
            JSON.stringify({
                contributor: contribution.contributor,
                taskId: contribution.taskId,
                computeUnits: contribution.computeUnits,
                credits: contribution.credits,
            }),
            contribution.signature
        );

        // Verify compute units are reasonable
        const reasonable = contribution.computeUnits > 0 &&
                          contribution.computeUnits < 1000000 &&
                          contribution.credits === Math.floor(contribution.computeUnits / 100);

        return {
            valid: valid && reasonable,
            reason: !valid ? 'Invalid signature' : (!reasonable ? 'Unreasonable values' : 'OK'),
        };
    }
}

/**
 * Network Manager - High-level API
 */
export class NetworkManager {
    constructor(identity) {
        this.identity = identity;
        this.peerManager = new PeerManager(identity);
        this.qdag = null;
        this.verifier = null;
        this.initialized = false;
    }

    async initialize() {
        console.log('\n🌐 Initializing Edge-Net Network...');

        await this.peerManager.initialize();

        this.qdag = new QDAGLedger(this.peerManager);
        await this.qdag.initialize();

        this.verifier = new ContributionVerifier(this.peerManager, this.qdag);

        // Announce to network
        await this.peerManager.announce();

        this.initialized = true;
        console.log('✅ Network initialized\n');

        return this;
    }

    /**
     * Record a compute contribution
     */
    async recordContribution(taskId, computeUnits) {
        const credits = Math.floor(computeUnits / 100);

        const contribution = {
            piKey: this.identity.piKey,
            siteId: this.identity.siteId,
            taskId,
            computeUnits,
            credits,
            timestamp: Date.now(),
        };

        return await this.verifier.submitContribution(contribution);
    }

    /**
     * Get stats for this contributor
     */
    getMyStats() {
        return this.qdag.getContributorStats(this.identity.piKey);
    }

    /**
     * Get network-wide stats
     */
    getNetworkStats() {
        return this.qdag.getNetworkStats();
    }

    /**
     * Get connected peers
     */
    getPeers() {
        return this.peerManager.getAllPeers();
    }

    /**
     * Stop network services
     */
    stop() {
        this.peerManager.stop();
        this.qdag.stop();
    }
}

// CLI interface
async function main() {
    const args = process.argv.slice(2);
    const command = args[0];

    if (command === 'stats') {
        // Show network stats
        await ensureDirectories();

        try {
            const data = await fs.readFile(getQDAGFile(), 'utf-8');
            const ledger = JSON.parse(data);

            console.log('\n📊 Edge-Net Network Statistics\n');
            console.log(`  Total Nodes:    ${ledger.nodes?.length || 0}`);
            console.log(`  Confirmed:      ${ledger.confirmed?.length || 0}`);
            console.log(`  Current Tips:   ${ledger.tips?.length || 0}`);

            const contributions = (ledger.nodes || []).filter(n => n.type === 'contribution');
            const contributors = new Set(contributions.map(c => c.contributor));

            console.log(`  Contributions:  ${contributions.length}`);
            console.log(`  Contributors:   ${contributors.size}`);
            console.log(`  Total Credits:  ${contributions.reduce((s, c) => s + (c.credits || 0), 0)}`);
            console.log();
        } catch (err) {
            console.log('No QDAG data found. Start contributing to initialize the network.');
        }
    } else if (command === 'peers') {
        // Show known peers
        await ensureDirectories();

        try {
            const data = await fs.readFile(getPeersFile(), 'utf-8');
            const peers = JSON.parse(data);

            console.log('\n👥 Known Peers\n');
            for (const peer of peers) {
                const status = (Date.now() - peer.lastSeen) < 300000 ? '🟢' : '⚪';
                console.log(`  ${status} ${peer.siteId} (π:${peer.piKey.slice(0, 8)})`);
                console.log(`     First seen: ${new Date(peer.firstSeen).toLocaleString()}`);
                console.log(`     Last seen:  ${new Date(peer.lastSeen).toLocaleString()}`);
                console.log(`     Verified:   ${peer.verified ? '✅' : '❌'}`);
                console.log();
            }
        } catch (err) {
            console.log('No peers found. Join the network to discover peers.');
        }
    } else if (command === 'help' || !command) {
        console.log(`
Edge-Net Network Module

Commands:
  stats     Show network statistics
  peers     Show known peers
  help      Show this help

The network module is used internally by the join CLI.
To join the network: npx edge-net-join --generate
        `);
    }
}

main().catch(console.error);
