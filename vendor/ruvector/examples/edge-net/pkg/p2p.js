/**
 * @ruvector/edge-net P2P Integration
 *
 * Unified P2P networking layer that integrates:
 * - WebRTC for direct peer connections
 * - DHT for decentralized peer discovery
 * - Signaling for connection establishment
 * - Sync for ledger synchronization
 *
 * @module @ruvector/edge-net/p2p
 */

import { EventEmitter } from 'events';
import { randomBytes } from 'crypto';

// Import P2P components
import { WebRTCPeerManager, WEBRTC_CONFIG } from './webrtc.js';
import { DHTNode, createDHTNode } from './dht.js';
import { SignalingClient } from './signaling.js';
import { SyncManager } from './sync.js';
import { QDAG } from './qdag.js';
import { Ledger } from './ledger.js';
import { HybridBootstrap, FirebaseLedgerSync } from './firebase-signaling.js';

// ============================================
// P2P NETWORK CONFIGURATION
// ============================================

export const P2P_CONFIG = {
    // Auto-start components
    autoStart: {
        signaling: true,
        webrtc: true,
        dht: true,
        sync: true,
        firebase: true,  // Use Firebase for bootstrap
    },
    // Bootstrap strategy: 'firebase' | 'local' | 'dht-only'
    bootstrapStrategy: 'firebase',
    // Connection settings
    maxPeers: 50,
    minPeers: 3,
    // Reconnection
    reconnectInterval: 5000,
    maxReconnectAttempts: 10,
    // DHT settings
    dhtAnnounceInterval: 60000,
    // Sync settings
    syncInterval: 30000,
    // Firebase settings (optional override)
    firebase: {
        // Default uses public edge-net Firebase
        // Override with your own project for production
        projectId: null,
        apiKey: null,
    },
    // Migration thresholds
    migration: {
        dhtPeerThreshold: 5,   // Peers needed before reducing Firebase dependency
        p2pPeerThreshold: 10,  // Peers needed for full P2P mode
    },
};

// ============================================
// P2P NETWORK NODE
// ============================================

/**
 * Unified P2P Network Node
 *
 * Connects all P2P components into a single interface:
 * - Automatic peer discovery via DHT
 * - WebRTC data channels for direct messaging
 * - QDAG for distributed consensus
 * - Ledger sync across devices
 */
export class P2PNetwork extends EventEmitter {
    constructor(identity, options = {}) {
        super();
        this.identity = identity;
        this.options = { ...P2P_CONFIG, ...options };

        // Node ID
        this.nodeId = identity?.piKey || identity?.nodeId || `node-${randomBytes(8).toString('hex')}`;

        // Components (initialized on start)
        this.signaling = null;
        this.webrtc = null;
        this.dht = null;
        this.syncManager = null;
        this.qdag = null;
        this.ledger = null;

        // Firebase bootstrap (Google Cloud)
        this.hybridBootstrap = null;
        this.firebaseLedgerSync = null;

        // State
        this.state = 'stopped';
        this.peers = new Map();
        this.stats = {
            startedAt: null,
            peersConnected: 0,
            messagesReceived: 0,
            messagesSent: 0,
            bytesTransferred: 0,
        };
    }

    /**
     * Start the P2P network
     */
    async start() {
        if (this.state === 'running') return this;

        console.log('\n🌐 Starting P2P Network...');
        console.log(`   Node ID: ${this.nodeId.slice(0, 16)}...`);

        this.state = 'starting';

        try {
            // Initialize QDAG
            this.qdag = new QDAG({ nodeId: this.nodeId });
            console.log('   ✅ QDAG initialized');

            // Initialize Ledger
            this.ledger = new Ledger({ nodeId: this.nodeId });
            console.log('   ✅ Ledger initialized');

            // Start Firebase bootstrap (Google Cloud) - Primary path
            if (this.options.autoStart.firebase && this.options.bootstrapStrategy === 'firebase') {
                await this.startFirebaseBootstrap();
            }
            // Fallback: Try local signaling server
            else if (this.options.autoStart.signaling) {
                await this.startSignaling();
            }

            // Initialize WebRTC
            if (this.options.autoStart.webrtc) {
                await this.startWebRTC();
            }

            // Initialize DHT for peer discovery
            if (this.options.autoStart.dht) {
                await this.startDHT();
            }

            // Initialize sync
            if (this.options.autoStart.sync && this.identity) {
                await this.startSync();
            }

            // Start Firebase hybrid bootstrap after WebRTC/DHT are ready
            if (this.hybridBootstrap && this.webrtc) {
                await this.hybridBootstrap.start(this.webrtc, this.dht);

                // Also start Firebase ledger sync
                await this.startFirebaseLedgerSync();
            }

            // Wire up event handlers
            this.setupEventHandlers();

            // Announce presence
            await this.announce();

            this.state = 'running';
            this.stats.startedAt = Date.now();

            console.log('\n✅ P2P Network running');
            console.log(`   Mode: ${this.getMode()}`);
            console.log(`   Peers: ${this.peers.size}`);

            this.emit('started', { nodeId: this.nodeId, mode: this.getMode() });

            return this;

        } catch (error) {
            this.state = 'error';
            console.error('❌ P2P Network start failed:', error.message);
            throw error;
        }
    }

    /**
     * Stop the P2P network
     */
    async stop() {
        console.log('\n🛑 Stopping P2P Network...');

        this.state = 'stopping';

        // Stop Firebase components
        if (this.hybridBootstrap) await this.hybridBootstrap.stop();
        if (this.firebaseLedgerSync) this.firebaseLedgerSync.stop();

        // Stop P2P components
        if (this.syncManager) this.syncManager.stop();
        if (this.dht) this.dht.stop();
        if (this.webrtc) this.webrtc.close();
        if (this.signaling) this.signaling.disconnect();

        this.peers.clear();
        this.state = 'stopped';

        this.emit('stopped');
    }

    /**
     * Start Firebase hybrid bootstrap (Google Cloud)
     * Primary bootstrap path - uses Firebase for discovery, migrates to P2P
     */
    async startFirebaseBootstrap() {
        try {
            this.hybridBootstrap = new HybridBootstrap({
                peerId: this.nodeId,
                firebaseConfig: this.options.firebase?.projectId ? {
                    apiKey: this.options.firebase.apiKey,
                    projectId: this.options.firebase.projectId,
                    authDomain: `${this.options.firebase.projectId}.firebaseapp.com`,
                    databaseURL: `https://${this.options.firebase.projectId}-default-rtdb.firebaseio.com`,
                } : undefined,
                dhtPeerThreshold: this.options.migration?.dhtPeerThreshold,
                p2pPeerThreshold: this.options.migration?.p2pPeerThreshold,
            });

            // Wire up bootstrap events
            this.hybridBootstrap.on('peer-discovered', ({ peerId, source }) => {
                console.log(`   🔍 Discovered peer via ${source}: ${peerId.slice(0, 8)}...`);
            });

            this.hybridBootstrap.on('mode-changed', ({ from, to }) => {
                console.log(`   🔄 Bootstrap mode: ${from} → ${to}`);
                this.emit('bootstrap-mode-changed', { from, to });
            });

            // Start will be called after WebRTC/DHT init
            console.log('   ✅ Firebase bootstrap initialized');

        } catch (err) {
            console.log('   ⚠️  Firebase bootstrap failed:', err.message);
            // Fall back to local signaling
            await this.startSignaling();
        }
    }

    /**
     * Start Firebase ledger sync
     */
    async startFirebaseLedgerSync() {
        if (!this.ledger) return;

        try {
            this.firebaseLedgerSync = new FirebaseLedgerSync(this.ledger, {
                peerId: this.nodeId,
                firebaseConfig: this.options.firebase?.projectId ? {
                    apiKey: this.options.firebase.apiKey,
                    projectId: this.options.firebase.projectId,
                } : undefined,
                syncInterval: this.options.syncInterval,
            });

            await this.firebaseLedgerSync.start();
            console.log('   ✅ Firebase ledger sync started');

        } catch (err) {
            console.log('   ⚠️  Firebase ledger sync failed:', err.message);
        }
    }

    /**
     * Start signaling client (fallback for local development)
     */
    async startSignaling() {
        try {
            this.signaling = new SignalingClient({
                serverUrl: this.options.signalingUrl || WEBRTC_CONFIG.signalingServers[0],
                peerId: this.nodeId,
            });

            const connected = await this.signaling.connect();
            if (connected) {
                console.log('   ✅ Signaling connected (local)');
            } else {
                console.log('   ⚠️  Signaling unavailable (will use DHT)');
            }
        } catch (err) {
            console.log('   ⚠️  Signaling failed:', err.message);
        }
    }

    /**
     * Start WebRTC peer manager
     */
    async startWebRTC() {
        try {
            this.webrtc = new WebRTCPeerManager({
                piKey: this.nodeId,
                siteId: this.identity?.siteId || 'edge-net',
            }, this.options.webrtc || {});

            await this.webrtc.initialize();
            console.log(`   ✅ WebRTC initialized (${this.webrtc.mode} mode)`);
        } catch (err) {
            console.log('   ⚠️  WebRTC failed:', err.message);
        }
    }

    /**
     * Start DHT for peer discovery
     */
    async startDHT() {
        try {
            if (this.webrtc) {
                this.dht = await createDHTNode(this.webrtc, {
                    id: this.nodeId,
                });
            } else {
                this.dht = new DHTNode({ id: this.nodeId });
                await this.dht.start();
            }
            console.log('   ✅ DHT initialized');
        } catch (err) {
            console.log('   ⚠️  DHT failed:', err.message);
        }
    }

    /**
     * Start sync manager
     */
    async startSync() {
        try {
            this.syncManager = new SyncManager(this.identity, this.ledger, {
                enableP2P: !!this.webrtc,
                enableFirestore: false, // Use P2P only for now
            });

            await this.syncManager.start();
            console.log('   ✅ Sync initialized');
        } catch (err) {
            console.log('   ⚠️  Sync failed:', err.message);
        }
    }

    /**
     * Setup event handlers between components
     */
    setupEventHandlers() {
        // WebRTC events
        if (this.webrtc) {
            this.webrtc.on('peer-connected', (peerId) => {
                this.handlePeerConnected(peerId);
            });

            this.webrtc.on('peer-disconnected', (peerId) => {
                this.handlePeerDisconnected(peerId);
            });

            this.webrtc.on('message', ({ from, message }) => {
                this.handleMessage(from, message);
            });
        }

        // Signaling events
        if (this.signaling) {
            this.signaling.on('peer-joined', ({ peerId }) => {
                this.connectToPeer(peerId);
            });

            this.signaling.on('offer', async ({ from, offer }) => {
                if (this.webrtc) {
                    await this.webrtc.handleOffer({ from, offer });
                }
            });

            this.signaling.on('answer', async ({ from, answer }) => {
                if (this.webrtc) {
                    await this.webrtc.handleAnswer({ from, answer });
                }
            });

            this.signaling.on('ice-candidate', async ({ from, candidate }) => {
                if (this.webrtc) {
                    await this.webrtc.handleIceCandidate({ from, candidate });
                }
            });
        }

        // DHT events
        if (this.dht) {
            this.dht.on('peer-added', (peer) => {
                this.connectToPeer(peer.id);
            });
        }

        // Sync events
        if (this.syncManager) {
            this.syncManager.on('synced', (data) => {
                this.emit('synced', data);
            });
        }

        // QDAG events
        if (this.qdag) {
            this.qdag.on('transaction', (tx) => {
                this.broadcastTransaction(tx);
            });

            this.qdag.on('confirmed', (tx) => {
                this.emit('transaction-confirmed', tx);
            });
        }
    }

    /**
     * Announce presence to the network
     */
    async announce() {
        // Announce via signaling
        if (this.signaling?.isConnected) {
            this.signaling.send({
                type: 'announce',
                piKey: this.nodeId,
                siteId: this.identity?.siteId,
                capabilities: ['compute', 'storage', 'verify'],
            });
        }

        // Announce via DHT
        if (this.dht) {
            await this.dht.announce('edge-net');
        }
    }

    /**
     * Connect to a peer
     */
    async connectToPeer(peerId) {
        if (peerId === this.nodeId) return;
        if (this.peers.has(peerId)) return;

        try {
            if (this.webrtc) {
                await this.webrtc.connectToPeer(peerId);
            }
        } catch (err) {
            console.warn(`[P2P] Failed to connect to ${peerId.slice(0, 8)}:`, err.message);
        }
    }

    /**
     * Handle peer connected
     */
    handlePeerConnected(peerId) {
        this.peers.set(peerId, {
            id: peerId,
            connectedAt: Date.now(),
            lastSeen: Date.now(),
        });

        this.stats.peersConnected++;

        // Register with sync
        if (this.syncManager && this.webrtc) {
            const peer = this.webrtc.peers.get(peerId);
            if (peer?.dataChannel) {
                this.syncManager.registerPeer(peerId, peer.dataChannel);
            }
        }

        this.emit('peer-connected', { peerId });
        console.log(`   🔗 Connected to peer: ${peerId.slice(0, 8)}... (${this.peers.size} total)`);
    }

    /**
     * Handle peer disconnected
     */
    handlePeerDisconnected(peerId) {
        this.peers.delete(peerId);
        this.emit('peer-disconnected', { peerId });
    }

    /**
     * Handle incoming message
     */
    handleMessage(from, message) {
        this.stats.messagesReceived++;

        // Route by message type
        switch (message.type) {
            case 'qdag_transaction':
                this.handleQDAGTransaction(from, message);
                break;

            case 'qdag_sync_request':
                this.handleQDAGSyncRequest(from, message);
                break;

            case 'qdag_sync_response':
                this.handleQDAGSyncResponse(from, message);
                break;

            default:
                this.emit('message', { from, message });
        }
    }

    /**
     * Handle QDAG transaction
     */
    handleQDAGTransaction(from, message) {
        if (message.transaction && this.qdag) {
            try {
                this.qdag.addTransaction(message.transaction);
            } catch (err) {
                // Duplicate or invalid transaction
            }
        }
    }

    /**
     * Handle QDAG sync request
     */
    handleQDAGSyncRequest(from, message) {
        if (this.qdag && this.webrtc) {
            const transactions = this.qdag.export(message.since || 0);
            this.webrtc.sendToPeer(from, {
                type: 'qdag_sync_response',
                transactions: transactions.transactions,
            });
        }
    }

    /**
     * Handle QDAG sync response
     */
    handleQDAGSyncResponse(from, message) {
        if (message.transactions && this.qdag) {
            this.qdag.import({ transactions: message.transactions });
        }
    }

    /**
     * Broadcast a transaction to all peers
     */
    broadcastTransaction(tx) {
        if (this.webrtc) {
            this.webrtc.broadcast({
                type: 'qdag_transaction',
                transaction: tx.toJSON(),
            });
            this.stats.messagesSent++;
        }
    }

    /**
     * Send message to a specific peer
     */
    sendToPeer(peerId, message) {
        if (this.webrtc) {
            const sent = this.webrtc.sendToPeer(peerId, message);
            if (sent) this.stats.messagesSent++;
            return sent;
        }
        return false;
    }

    /**
     * Broadcast message to all peers
     */
    broadcast(message) {
        if (this.webrtc) {
            const sent = this.webrtc.broadcast(message);
            this.stats.messagesSent += sent;
            return sent;
        }
        return 0;
    }

    /**
     * Submit a task to the network
     */
    async submitTask(task) {
        if (!this.qdag) throw new Error('QDAG not initialized');

        const tx = this.qdag.createTransaction('task', {
            taskId: task.id || randomBytes(8).toString('hex'),
            type: task.type,
            data: task.data,
            reward: task.reward || 0,
            priority: task.priority || 'medium',
        }, {
            issuer: this.nodeId,
        });

        return tx;
    }

    /**
     * Credit the ledger
     */
    credit(amount, reason) {
        if (!this.ledger) throw new Error('Ledger not initialized');
        this.ledger.credit(amount, reason);

        // Trigger sync
        if (this.syncManager) {
            this.syncManager.sync();
        }
    }

    /**
     * Get current balance
     */
    getBalance() {
        return this.ledger?.balance?.() ?? this.ledger?.getBalance?.() ?? 0;
    }

    /**
     * Get connection mode
     * Returns the current bootstrap/connectivity mode
     */
    getMode() {
        // Check Firebase hybrid bootstrap mode first
        if (this.hybridBootstrap) {
            const bootstrapMode = this.hybridBootstrap.mode;
            if (bootstrapMode === 'p2p') return 'full-p2p';
            if (bootstrapMode === 'hybrid') return 'firebase-hybrid';
            if (bootstrapMode === 'firebase') return 'firebase-bootstrap';
        }

        // Legacy mode detection
        if (this.webrtc?.mode === 'webrtc' && this.signaling?.isConnected) {
            return 'full-p2p';
        }
        if (this.webrtc?.mode === 'webrtc') {
            return 'webrtc-dht';
        }
        if (this.dht) {
            return 'dht-only';
        }
        return 'local';
    }

    /**
     * Get bootstrap mode (firebase/hybrid/p2p)
     */
    getBootstrapMode() {
        return this.hybridBootstrap?.mode || 'none';
    }

    /**
     * Get network statistics
     */
    getStats() {
        const bootstrapStats = this.hybridBootstrap?.getStats() || {};

        return {
            ...this.stats,
            nodeId: this.nodeId,
            state: this.state,
            mode: this.getMode(),
            bootstrapMode: this.getBootstrapMode(),
            peers: this.peers.size,
            // Firebase stats
            firebaseConnected: bootstrapStats.firebaseConnected || false,
            firebasePeers: bootstrapStats.firebasePeers || 0,
            firebaseSignals: bootstrapStats.firebaseSignals || 0,
            p2pSignals: bootstrapStats.p2pSignals || 0,
            // Legacy signaling
            signalingConnected: this.signaling?.isConnected || false,
            webrtcMode: this.webrtc?.mode || 'none',
            dhtPeers: this.dht?.getPeers().length || 0,
            qdagTransactions: this.qdag?.transactions.size || 0,
            ledgerBalance: this.getBalance(),
            uptime: this.stats.startedAt ? Date.now() - this.stats.startedAt : 0,
        };
    }

    /**
     * Get peer list
     */
    getPeers() {
        return Array.from(this.peers.values());
    }
}

/**
 * Create and start a P2P network node
 */
export async function createP2PNetwork(identity, options = {}) {
    const network = new P2PNetwork(identity, options);
    await network.start();
    return network;
}

// ============================================
// EXPORTS
// ============================================

export default P2PNetwork;
