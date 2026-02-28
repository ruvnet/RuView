/**
 * @ruvector/edge-net Hybrid Sync Service
 *
 * Multi-device identity and ledger synchronization using:
 * - P2P sync via WebRTC (fast, direct when devices online together)
 * - Firestore sync (persistent fallback, cross-session)
 * - Identity linking via PiKey signatures
 *
 * @module @ruvector/edge-net/sync
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';

// ============================================
// SYNC CONFIGURATION
// ============================================

export const SYNC_CONFIG = {
    // Firestore endpoints (Genesis nodes)
    firestore: {
        projectId: 'ruvector-edge-net',
        collection: 'ledger-sync',
        identityCollection: 'identity-links',
    },
    // Sync intervals
    intervals: {
        p2pHeartbeat: 5000,      // 5s P2P sync check
        firestoreSync: 30000,    // 30s Firestore sync
        staleThreshold: 60000,   // 1min before considering state stale
    },
    // CRDT merge settings
    crdt: {
        maxBatchSize: 1000,      // Max entries per merge
        conflictResolution: 'lww', // Last-write-wins
    },
    // Genesis node endpoints
    genesisNodes: [
        { region: 'us-central1', url: 'https://edge-net-genesis-us.ruvector.dev' },
        { region: 'europe-west1', url: 'https://edge-net-genesis-eu.ruvector.dev' },
        { region: 'asia-east1', url: 'https://edge-net-genesis-asia.ruvector.dev' },
    ],
};

// ============================================
// IDENTITY LINKER
// ============================================

/**
 * Links a PiKey identity across multiple devices
 * Uses cryptographic challenge-response to prove ownership
 */
export class IdentityLinker extends EventEmitter {
    constructor(piKey, options = {}) {
        super();
        this.piKey = piKey;
        this.publicKeyHex = this.toHex(piKey.getPublicKey());
        this.shortId = piKey.getShortId();
        this.options = {
            genesisUrl: options.genesisUrl || SYNC_CONFIG.genesisNodes[0].url,
            ...options,
        };
        this.linkedDevices = new Map();
        this.authToken = null;
        this.deviceId = this.generateDeviceId();
    }

    /**
     * Generate unique device ID
     */
    generateDeviceId() {
        const platform = typeof window !== 'undefined' ? 'browser' : 'node';
        const random = randomBytes(8).toString('hex');
        const timestamp = Date.now().toString(36);
        return `${platform}-${timestamp}-${random}`;
    }

    /**
     * Authenticate with genesis node using PiKey signature
     */
    async authenticate() {
        try {
            // Step 1: Request challenge
            const challengeRes = await this.fetchWithTimeout(
                `${this.options.genesisUrl}/api/v1/identity/challenge`,
                {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        publicKey: this.publicKeyHex,
                        deviceId: this.deviceId,
                    }),
                }
            );

            if (!challengeRes.ok) {
                throw new Error(`Challenge request failed: ${challengeRes.status}`);
            }

            const { challenge, nonce } = await challengeRes.json();

            // Step 2: Sign challenge with PiKey
            const challengeBytes = this.fromHex(challenge);
            const signature = this.piKey.sign(challengeBytes);

            // Step 3: Submit signature for verification
            const authRes = await this.fetchWithTimeout(
                `${this.options.genesisUrl}/api/v1/identity/verify`,
                {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        publicKey: this.publicKeyHex,
                        deviceId: this.deviceId,
                        nonce,
                        signature: this.toHex(signature),
                    }),
                }
            );

            if (!authRes.ok) {
                throw new Error(`Authentication failed: ${authRes.status}`);
            }

            const { token, expiresAt, linkedDevices } = await authRes.json();

            this.authToken = token;
            this.tokenExpiry = new Date(expiresAt);

            // Update linked devices
            for (const device of linkedDevices || []) {
                this.linkedDevices.set(device.deviceId, device);
            }

            this.emit('authenticated', {
                deviceId: this.deviceId,
                linkedDevices: this.linkedDevices.size,
            });

            return { success: true, token, linkedDevices: this.linkedDevices.size };

        } catch (error) {
            // Fallback: Generate local-only token for P2P sync
            console.warn('[Sync] Genesis authentication failed, using local mode:', error.message);
            this.authToken = this.generateLocalToken();
            this.emit('authenticated', { deviceId: this.deviceId, mode: 'local' });
            return { success: true, mode: 'local' };
        }
    }

    /**
     * Generate local token for P2P-only mode
     */
    generateLocalToken() {
        const payload = {
            sub: this.publicKeyHex,
            dev: this.deviceId,
            iat: Date.now(),
            mode: 'local',
        };
        return Buffer.from(JSON.stringify(payload)).toString('base64');
    }

    /**
     * Link a new device to this identity
     */
    async linkDevice(deviceInfo) {
        if (!this.authToken) {
            await this.authenticate();
        }

        try {
            const res = await this.fetchWithTimeout(
                `${this.options.genesisUrl}/api/v1/identity/link`,
                {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'Authorization': `Bearer ${this.authToken}`,
                    },
                    body: JSON.stringify({
                        publicKey: this.publicKeyHex,
                        newDevice: deviceInfo,
                    }),
                }
            );

            if (!res.ok) {
                throw new Error(`Link failed: ${res.status}`);
            }

            const result = await res.json();
            this.linkedDevices.set(deviceInfo.deviceId, deviceInfo);

            this.emit('device_linked', { deviceId: deviceInfo.deviceId });
            return result;

        } catch (error) {
            // P2P fallback: Store in local linked devices for gossip
            this.linkedDevices.set(deviceInfo.deviceId, {
                ...deviceInfo,
                linkedAt: Date.now(),
                mode: 'p2p',
            });
            return { success: true, mode: 'p2p' };
        }
    }

    /**
     * Get all linked devices
     */
    getLinkedDevices() {
        return Array.from(this.linkedDevices.values());
    }

    /**
     * Check if a device is linked to this identity
     */
    isDeviceLinked(deviceId) {
        return this.linkedDevices.has(deviceId);
    }

    // Utility methods
    toHex(bytes) {
        return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
    }

    fromHex(hex) {
        const bytes = new Uint8Array(hex.length / 2);
        for (let i = 0; i < hex.length; i += 2) {
            bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
        }
        return bytes;
    }

    async fetchWithTimeout(url, options, timeout = 10000) {
        const controller = new AbortController();
        const id = setTimeout(() => controller.abort(), timeout);
        try {
            const response = await fetch(url, { ...options, signal: controller.signal });
            clearTimeout(id);
            return response;
        } catch (error) {
            clearTimeout(id);
            throw error;
        }
    }
}

// ============================================
// LEDGER SYNC SERVICE
// ============================================

/**
 * Hybrid sync service for credit ledger
 * Combines P2P (fast) and Firestore (persistent) sync
 */
export class LedgerSyncService extends EventEmitter {
    constructor(identityLinker, ledger, options = {}) {
        super();
        this.identity = identityLinker;
        this.ledger = ledger;
        this.options = {
            enableP2P: true,
            enableFirestore: true,
            syncInterval: SYNC_CONFIG.intervals.firestoreSync,
            ...options,
        };

        // Sync state
        this.lastSyncTime = 0;
        this.syncInProgress = false;
        this.pendingChanges = [];
        this.peerStates = new Map();  // deviceId -> { earned, spent, timestamp }
        this.vectorClock = new Map(); // deviceId -> counter

        // P2P connections
        this.p2pPeers = new Map();

        // Intervals
        this.syncIntervalId = null;
        this.heartbeatId = null;
    }

    /**
     * Start sync service
     */
    async start() {
        // Authenticate first
        await this.identity.authenticate();

        // Start periodic sync
        if (this.options.enableFirestore) {
            this.syncIntervalId = setInterval(
                () => this.syncWithFirestore(),
                this.options.syncInterval
            );
        }

        // Start P2P heartbeat
        if (this.options.enableP2P) {
            this.heartbeatId = setInterval(
                () => this.p2pHeartbeat(),
                SYNC_CONFIG.intervals.p2pHeartbeat
            );
        }

        // Initial sync
        await this.fullSync();

        this.emit('started', { deviceId: this.identity.deviceId });
        return this;
    }

    /**
     * Stop sync service
     */
    stop() {
        if (this.syncIntervalId) {
            clearInterval(this.syncIntervalId);
            this.syncIntervalId = null;
        }
        if (this.heartbeatId) {
            clearInterval(this.heartbeatId);
            this.heartbeatId = null;
        }
        this.emit('stopped');
    }

    /**
     * Full sync - fetch from all sources and merge
     */
    async fullSync() {
        if (this.syncInProgress) return;
        this.syncInProgress = true;

        try {
            const results = await Promise.allSettled([
                this.options.enableFirestore ? this.fetchFromFirestore() : null,
                this.options.enableP2P ? this.fetchFromP2PPeers() : null,
            ]);

            // Merge all fetched states
            for (const result of results) {
                if (result.status === 'fulfilled' && result.value) {
                    await this.mergeState(result.value);
                }
            }

            // Push our state
            await this.pushState();

            this.lastSyncTime = Date.now();
            this.emit('synced', {
                timestamp: this.lastSyncTime,
                balance: this.ledger.balance(),
            });

        } catch (error) {
            this.emit('sync_error', { error: error.message });
        } finally {
            this.syncInProgress = false;
        }
    }

    /**
     * Fetch ledger state from Firestore
     */
    async fetchFromFirestore() {
        if (!this.identity.authToken) return null;

        try {
            const res = await this.identity.fetchWithTimeout(
                `${this.identity.options.genesisUrl}/api/v1/ledger/${this.identity.publicKeyHex}`,
                {
                    method: 'GET',
                    headers: {
                        'Authorization': `Bearer ${this.identity.authToken}`,
                    },
                }
            );

            if (!res.ok) {
                if (res.status === 404) return null; // No state yet
                throw new Error(`Firestore fetch failed: ${res.status}`);
            }

            const { states } = await res.json();
            return states; // Array of { deviceId, earned, spent, timestamp }

        } catch (error) {
            console.warn('[Sync] Firestore fetch failed:', error.message);
            return null;
        }
    }

    /**
     * Fetch ledger state from P2P peers
     */
    async fetchFromP2PPeers() {
        const states = [];

        for (const [peerId, peer] of this.p2pPeers) {
            try {
                if (peer.dataChannel?.readyState === 'open') {
                    const state = await this.requestStateFromPeer(peer);
                    if (state) {
                        states.push({ deviceId: peerId, ...state });
                    }
                }
            } catch (error) {
                console.warn(`[Sync] P2P fetch from ${peerId} failed:`, error.message);
            }
        }

        return states.length > 0 ? states : null;
    }

    /**
     * Request state from a P2P peer
     */
    requestStateFromPeer(peer) {
        return new Promise((resolve, reject) => {
            const requestId = randomBytes(8).toString('hex');
            const timeout = setTimeout(() => {
                reject(new Error('P2P state request timeout'));
            }, 5000);

            const handler = (event) => {
                try {
                    const msg = JSON.parse(event.data);
                    if (msg.type === 'ledger_state' && msg.requestId === requestId) {
                        clearTimeout(timeout);
                        peer.dataChannel.removeEventListener('message', handler);
                        resolve(msg.state);
                    }
                } catch (e) { /* ignore */ }
            };

            peer.dataChannel.addEventListener('message', handler);
            peer.dataChannel.send(JSON.stringify({
                type: 'ledger_state_request',
                requestId,
                from: this.identity.deviceId,
            }));
        });
    }

    /**
     * Merge remote state into local ledger (CRDT)
     */
    async mergeState(states) {
        if (!states || !Array.isArray(states)) return;

        for (const state of states) {
            // Skip our own state
            if (state.deviceId === this.identity.deviceId) continue;

            // Check vector clock for freshness
            const lastSeen = this.vectorClock.get(state.deviceId) || 0;
            if (state.timestamp <= lastSeen) continue;

            // CRDT merge
            try {
                if (state.earned && state.spent) {
                    const earned = typeof state.earned === 'string'
                        ? JSON.parse(state.earned)
                        : state.earned;
                    const spent = typeof state.spent === 'string'
                        ? JSON.parse(state.spent)
                        : state.spent;

                    this.ledger.merge(
                        JSON.stringify(earned),
                        JSON.stringify(spent)
                    );
                }

                // Update vector clock
                this.vectorClock.set(state.deviceId, state.timestamp);
                this.peerStates.set(state.deviceId, state);

                this.emit('state_merged', {
                    deviceId: state.deviceId,
                    newBalance: this.ledger.balance(),
                });

            } catch (error) {
                console.warn(`[Sync] Merge failed for ${state.deviceId}:`, error.message);
            }
        }
    }

    /**
     * Push local state to sync destinations
     */
    async pushState() {
        const state = this.exportState();

        // Push to Firestore
        if (this.options.enableFirestore && this.identity.authToken) {
            await this.pushToFirestore(state);
        }

        // Broadcast to P2P peers
        if (this.options.enableP2P) {
            this.broadcastToP2P(state);
        }
    }

    /**
     * Export current ledger state
     */
    exportState() {
        return {
            deviceId: this.identity.deviceId,
            publicKey: this.identity.publicKeyHex,
            earned: this.ledger.exportEarned(),
            spent: this.ledger.exportSpent(),
            balance: this.ledger.balance(),
            totalEarned: this.ledger.totalEarned(),
            totalSpent: this.ledger.totalSpent(),
            timestamp: Date.now(),
        };
    }

    /**
     * Push state to Firestore
     */
    async pushToFirestore(state) {
        try {
            const res = await this.identity.fetchWithTimeout(
                `${this.identity.options.genesisUrl}/api/v1/ledger/${this.identity.publicKeyHex}`,
                {
                    method: 'PUT',
                    headers: {
                        'Content-Type': 'application/json',
                        'Authorization': `Bearer ${this.identity.authToken}`,
                    },
                    body: JSON.stringify({
                        deviceId: state.deviceId,
                        earned: state.earned,
                        spent: state.spent,
                        timestamp: state.timestamp,
                    }),
                }
            );

            if (!res.ok) {
                throw new Error(`Firestore push failed: ${res.status}`);
            }

            return true;

        } catch (error) {
            console.warn('[Sync] Firestore push failed:', error.message);
            return false;
        }
    }

    /**
     * Broadcast state to P2P peers
     */
    broadcastToP2P(state) {
        const message = JSON.stringify({
            type: 'ledger_state_broadcast',
            state: {
                deviceId: state.deviceId,
                earned: state.earned,
                spent: state.spent,
                timestamp: state.timestamp,
            },
        });

        for (const [peerId, peer] of this.p2pPeers) {
            try {
                if (peer.dataChannel?.readyState === 'open') {
                    peer.dataChannel.send(message);
                }
            } catch (error) {
                console.warn(`[Sync] P2P broadcast to ${peerId} failed:`, error.message);
            }
        }
    }

    /**
     * P2P heartbeat - discover and sync with nearby devices
     */
    async p2pHeartbeat() {
        // Broadcast presence to linked devices
        const presence = {
            type: 'presence',
            deviceId: this.identity.deviceId,
            publicKey: this.identity.publicKeyHex,
            balance: this.ledger.balance(),
            timestamp: Date.now(),
        };

        for (const [peerId, peer] of this.p2pPeers) {
            try {
                if (peer.dataChannel?.readyState === 'open') {
                    peer.dataChannel.send(JSON.stringify(presence));
                }
            } catch (error) {
                // Remove stale peer
                this.p2pPeers.delete(peerId);
            }
        }
    }

    /**
     * Register a P2P peer for sync
     */
    registerP2PPeer(peerId, dataChannel) {
        this.p2pPeers.set(peerId, { dataChannel, connectedAt: Date.now() });

        // Handle incoming messages
        dataChannel.addEventListener('message', (event) => {
            this.handleP2PMessage(peerId, event.data);
        });

        this.emit('peer_registered', { peerId });
    }

    /**
     * Handle incoming P2P message
     */
    async handleP2PMessage(peerId, data) {
        try {
            const msg = JSON.parse(data);

            switch (msg.type) {
                case 'ledger_state_request':
                    // Respond with our state
                    const state = this.exportState();
                    const peer = this.p2pPeers.get(peerId);
                    if (peer?.dataChannel?.readyState === 'open') {
                        peer.dataChannel.send(JSON.stringify({
                            type: 'ledger_state',
                            requestId: msg.requestId,
                            state: {
                                earned: state.earned,
                                spent: state.spent,
                                timestamp: state.timestamp,
                            },
                        }));
                    }
                    break;

                case 'ledger_state_broadcast':
                    // Merge incoming state
                    if (msg.state) {
                        await this.mergeState([{ deviceId: peerId, ...msg.state }]);
                    }
                    break;

                case 'presence':
                    // Update peer info
                    const existingPeer = this.p2pPeers.get(peerId);
                    if (existingPeer) {
                        existingPeer.lastSeen = Date.now();
                        existingPeer.balance = msg.balance;
                    }
                    break;
            }

        } catch (error) {
            console.warn(`[Sync] P2P message handling failed:`, error.message);
        }
    }

    /**
     * Sync with Firestore (called periodically)
     */
    async syncWithFirestore() {
        if (this.syncInProgress) return;

        try {
            const states = await this.fetchFromFirestore();
            if (states) {
                await this.mergeState(states);
            }
            await this.pushToFirestore(this.exportState());
        } catch (error) {
            console.warn('[Sync] Periodic Firestore sync failed:', error.message);
        }
    }

    /**
     * Force immediate sync
     */
    async forceSync() {
        return this.fullSync();
    }

    /**
     * Get sync status
     */
    getStatus() {
        return {
            deviceId: this.identity.deviceId,
            publicKey: this.identity.publicKeyHex,
            shortId: this.identity.shortId,
            linkedDevices: this.identity.getLinkedDevices().length,
            p2pPeers: this.p2pPeers.size,
            lastSyncTime: this.lastSyncTime,
            balance: this.ledger.balance(),
            totalEarned: this.ledger.totalEarned(),
            totalSpent: this.ledger.totalSpent(),
            syncEnabled: {
                p2p: this.options.enableP2P,
                firestore: this.options.enableFirestore,
            },
        };
    }
}

// ============================================
// SYNC MANAGER (CONVENIENCE WRAPPER)
// ============================================

/**
 * High-level sync manager for easy integration
 */
export class SyncManager extends EventEmitter {
    constructor(piKey, ledger, options = {}) {
        super();
        this.identityLinker = new IdentityLinker(piKey, options);
        this.syncService = new LedgerSyncService(this.identityLinker, ledger, options);

        // Forward events
        this.syncService.on('synced', (data) => this.emit('synced', data));
        this.syncService.on('state_merged', (data) => this.emit('state_merged', data));
        this.syncService.on('sync_error', (data) => this.emit('sync_error', data));
        this.identityLinker.on('authenticated', (data) => this.emit('authenticated', data));
        this.identityLinker.on('device_linked', (data) => this.emit('device_linked', data));
    }

    /**
     * Start sync
     */
    async start() {
        await this.syncService.start();
        return this;
    }

    /**
     * Stop sync
     */
    stop() {
        this.syncService.stop();
    }

    /**
     * Force sync
     */
    async sync() {
        return this.syncService.forceSync();
    }

    /**
     * Register P2P peer
     */
    registerPeer(peerId, dataChannel) {
        this.syncService.registerP2PPeer(peerId, dataChannel);
    }

    /**
     * Get status
     */
    getStatus() {
        return this.syncService.getStatus();
    }

    /**
     * Export identity for another device
     */
    exportIdentity(password) {
        return this.identityLinker.piKey.createEncryptedBackup(password);
    }

    /**
     * Link devices via QR code data
     */
    generateLinkData() {
        return {
            publicKey: this.identityLinker.publicKeyHex,
            shortId: this.identityLinker.shortId,
            genesisUrl: this.identityLinker.options.genesisUrl,
            timestamp: Date.now(),
        };
    }
}

// ============================================
// EXPORTS
// ============================================

export default SyncManager;
