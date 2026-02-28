#!/usr/bin/env node
/**
 * @ruvector/edge-net Genesis Node
 *
 * Bootstrap node for the edge-net P2P network.
 * Provides signaling, peer discovery, and ledger sync.
 *
 * Run: node genesis.js [--port 8787] [--data ~/.ruvector/genesis]
 *
 * @module @ruvector/edge-net/genesis
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join } from 'path';

// ============================================
// GENESIS NODE CONFIGURATION
// ============================================

export const GENESIS_CONFIG = {
    port: parseInt(process.env.GENESIS_PORT || '8787'),
    host: process.env.GENESIS_HOST || '0.0.0.0',
    dataDir: process.env.GENESIS_DATA || join(process.env.HOME || '/tmp', '.ruvector', 'genesis'),
    // Rate limiting
    rateLimit: {
        maxConnectionsPerIp: 50,
        maxMessagesPerSecond: 100,
        challengeExpiry: 60000, // 1 minute
    },
    // Cleanup
    cleanup: {
        staleConnectionTimeout: 300000, // 5 minutes
        cleanupInterval: 60000, // 1 minute
    },
};

// ============================================
// PEER REGISTRY
// ============================================

export class PeerRegistry {
    constructor() {
        this.peers = new Map();          // peerId -> peer info
        this.byPublicKey = new Map();    // publicKey -> peerId
        this.byRoom = new Map();         // room -> Set<peerId>
        this.connections = new Map();    // connectionId -> peerId
    }

    register(peerId, info) {
        this.peers.set(peerId, {
            ...info,
            peerId,
            registeredAt: Date.now(),
            lastSeen: Date.now(),
        });

        if (info.publicKey) {
            this.byPublicKey.set(info.publicKey, peerId);
        }

        return this.peers.get(peerId);
    }

    update(peerId, updates) {
        const peer = this.peers.get(peerId);
        if (peer) {
            Object.assign(peer, updates, { lastSeen: Date.now() });
        }
        return peer;
    }

    get(peerId) {
        return this.peers.get(peerId);
    }

    getByPublicKey(publicKey) {
        const peerId = this.byPublicKey.get(publicKey);
        return peerId ? this.peers.get(peerId) : null;
    }

    remove(peerId) {
        const peer = this.peers.get(peerId);
        if (peer) {
            if (peer.publicKey) {
                this.byPublicKey.delete(peer.publicKey);
            }
            if (peer.room) {
                const room = this.byRoom.get(peer.room);
                if (room) room.delete(peerId);
            }
            this.peers.delete(peerId);
            return true;
        }
        return false;
    }

    joinRoom(peerId, room) {
        const peer = this.peers.get(peerId);
        if (!peer) return false;

        // Leave old room
        if (peer.room && peer.room !== room) {
            const oldRoom = this.byRoom.get(peer.room);
            if (oldRoom) oldRoom.delete(peerId);
        }

        // Join new room
        if (!this.byRoom.has(room)) {
            this.byRoom.set(room, new Set());
        }
        this.byRoom.get(room).add(peerId);
        peer.room = room;

        return true;
    }

    getRoomPeers(room) {
        const peerIds = this.byRoom.get(room) || new Set();
        return Array.from(peerIds).map(id => this.peers.get(id)).filter(Boolean);
    }

    getAllPeers() {
        return Array.from(this.peers.values());
    }

    pruneStale(maxAge = GENESIS_CONFIG.cleanup.staleConnectionTimeout) {
        const cutoff = Date.now() - maxAge;
        const removed = [];

        for (const [peerId, peer] of this.peers) {
            if (peer.lastSeen < cutoff) {
                this.remove(peerId);
                removed.push(peerId);
            }
        }

        return removed;
    }

    getStats() {
        return {
            totalPeers: this.peers.size,
            rooms: this.byRoom.size,
            roomSizes: Object.fromEntries(
                Array.from(this.byRoom.entries()).map(([room, peers]) => [room, peers.size])
            ),
        };
    }
}

// ============================================
// LEDGER STORE
// ============================================

export class LedgerStore {
    constructor(dataDir) {
        this.dataDir = dataDir;
        this.ledgers = new Map();
        this.pendingWrites = new Map();

        // Ensure data directory exists
        if (!existsSync(dataDir)) {
            mkdirSync(dataDir, { recursive: true });
        }

        // Load existing ledgers
        this.loadAll();
    }

    loadAll() {
        try {
            const indexPath = join(this.dataDir, 'index.json');
            if (existsSync(indexPath)) {
                const index = JSON.parse(readFileSync(indexPath, 'utf8'));
                for (const publicKey of index.keys || []) {
                    this.load(publicKey);
                }
            }
        } catch (err) {
            console.warn('[Genesis] Failed to load ledger index:', err.message);
        }
    }

    load(publicKey) {
        try {
            const path = join(this.dataDir, `ledger-${publicKey.slice(0, 16)}.json`);
            if (existsSync(path)) {
                const data = JSON.parse(readFileSync(path, 'utf8'));
                this.ledgers.set(publicKey, data);
                return data;
            }
        } catch (err) {
            console.warn(`[Genesis] Failed to load ledger ${publicKey.slice(0, 8)}:`, err.message);
        }
        return null;
    }

    save(publicKey) {
        try {
            const data = this.ledgers.get(publicKey);
            if (!data) return false;

            const path = join(this.dataDir, `ledger-${publicKey.slice(0, 16)}.json`);
            writeFileSync(path, JSON.stringify(data, null, 2));

            // Update index
            this.saveIndex();
            return true;
        } catch (err) {
            console.warn(`[Genesis] Failed to save ledger ${publicKey.slice(0, 8)}:`, err.message);
            return false;
        }
    }

    saveIndex() {
        try {
            const indexPath = join(this.dataDir, 'index.json');
            writeFileSync(indexPath, JSON.stringify({
                keys: Array.from(this.ledgers.keys()),
                updatedAt: Date.now(),
            }, null, 2));
        } catch (err) {
            console.warn('[Genesis] Failed to save index:', err.message);
        }
    }

    get(publicKey) {
        return this.ledgers.get(publicKey);
    }

    getStates(publicKey) {
        const ledger = this.ledgers.get(publicKey);
        if (!ledger) return [];

        return Object.values(ledger.devices || {});
    }

    update(publicKey, deviceId, state) {
        if (!this.ledgers.has(publicKey)) {
            this.ledgers.set(publicKey, {
                publicKey,
                createdAt: Date.now(),
                devices: {},
            });
        }

        const ledger = this.ledgers.get(publicKey);

        // Merge state
        const existing = ledger.devices[deviceId] || {};
        const merged = this.mergeCRDT(existing, state);

        ledger.devices[deviceId] = {
            ...merged,
            deviceId,
            updatedAt: Date.now(),
        };

        // Schedule write
        this.scheduleSave(publicKey);

        return ledger.devices[deviceId];
    }

    mergeCRDT(existing, incoming) {
        // Simple LWW merge for now
        if (!existing.timestamp || incoming.timestamp > existing.timestamp) {
            return { ...incoming };
        }

        // If same timestamp, merge counters
        return {
            earned: Math.max(existing.earned || 0, incoming.earned || 0),
            spent: Math.max(existing.spent || 0, incoming.spent || 0),
            timestamp: Math.max(existing.timestamp || 0, incoming.timestamp || 0),
        };
    }

    scheduleSave(publicKey) {
        if (this.pendingWrites.has(publicKey)) return;

        this.pendingWrites.set(publicKey, setTimeout(() => {
            this.save(publicKey);
            this.pendingWrites.delete(publicKey);
        }, 1000));
    }

    flush() {
        for (const [publicKey, timeout] of this.pendingWrites) {
            clearTimeout(timeout);
            this.save(publicKey);
        }
        this.pendingWrites.clear();
    }

    getStats() {
        return {
            totalLedgers: this.ledgers.size,
            totalDevices: Array.from(this.ledgers.values())
                .reduce((sum, l) => sum + Object.keys(l.devices || {}).length, 0),
        };
    }
}

// ============================================
// AUTHENTICATION SERVICE
// ============================================

export class AuthService {
    constructor() {
        this.challenges = new Map();  // nonce -> { challenge, publicKey, expiresAt }
        this.tokens = new Map();      // token -> { publicKey, deviceId, expiresAt }
    }

    createChallenge(publicKey, deviceId) {
        const nonce = randomBytes(32).toString('hex');
        const challenge = randomBytes(32).toString('hex');

        this.challenges.set(nonce, {
            challenge,
            publicKey,
            deviceId,
            expiresAt: Date.now() + GENESIS_CONFIG.rateLimit.challengeExpiry,
        });

        return { nonce, challenge };
    }

    verifyChallenge(nonce, publicKey, signature) {
        const challengeData = this.challenges.get(nonce);
        if (!challengeData) {
            return { valid: false, error: 'Invalid nonce' };
        }

        if (Date.now() > challengeData.expiresAt) {
            this.challenges.delete(nonce);
            return { valid: false, error: 'Challenge expired' };
        }

        if (challengeData.publicKey !== publicKey) {
            return { valid: false, error: 'Public key mismatch' };
        }

        // Simple signature verification (in production, use proper Ed25519)
        const expectedSig = createHash('sha256')
            .update(challengeData.challenge + publicKey)
            .digest('hex');

        // For now, accept any signature (real impl would verify Ed25519)
        // In production: verify Ed25519 signature

        this.challenges.delete(nonce);

        // Generate token
        const token = randomBytes(32).toString('hex');
        const tokenData = {
            publicKey,
            deviceId: challengeData.deviceId,
            createdAt: Date.now(),
            expiresAt: Date.now() + 24 * 60 * 60 * 1000, // 24 hours
        };

        this.tokens.set(token, tokenData);

        return { valid: true, token, expiresAt: tokenData.expiresAt };
    }

    validateToken(token) {
        const tokenData = this.tokens.get(token);
        if (!tokenData) return null;

        if (Date.now() > tokenData.expiresAt) {
            this.tokens.delete(token);
            return null;
        }

        return tokenData;
    }

    cleanup() {
        const now = Date.now();

        for (const [nonce, data] of this.challenges) {
            if (now > data.expiresAt) {
                this.challenges.delete(nonce);
            }
        }

        for (const [token, data] of this.tokens) {
            if (now > data.expiresAt) {
                this.tokens.delete(token);
            }
        }
    }
}

// ============================================
// GENESIS NODE SERVER
// ============================================

export class GenesisNode extends EventEmitter {
    constructor(options = {}) {
        super();
        this.config = { ...GENESIS_CONFIG, ...options };
        this.peerRegistry = new PeerRegistry();
        this.ledgerStore = new LedgerStore(this.config.dataDir);
        this.authService = new AuthService();

        this.wss = null;
        this.connections = new Map();
        this.cleanupInterval = null;

        this.stats = {
            startedAt: null,
            totalConnections: 0,
            totalMessages: 0,
            signalsRelayed: 0,
        };
    }

    async start() {
        console.log('\n🌐 Starting Edge-Net Genesis Node...');
        console.log(`   Port: ${this.config.port}`);
        console.log(`   Data: ${this.config.dataDir}`);

        // Import ws dynamically
        const { WebSocketServer } = await import('ws');

        this.wss = new WebSocketServer({
            port: this.config.port,
            host: this.config.host,
        });

        this.wss.on('connection', (ws, req) => this.handleConnection(ws, req));
        this.wss.on('error', (err) => this.emit('error', err));

        // Start cleanup interval
        this.cleanupInterval = setInterval(() => this.cleanup(), this.config.cleanup.cleanupInterval);

        this.stats.startedAt = Date.now();

        console.log(`\n✅ Genesis Node running on ws://${this.config.host}:${this.config.port}`);
        console.log(`   API: http://${this.config.host}:${this.config.port}/api/v1/`);

        this.emit('started', { port: this.config.port });

        return this;
    }

    stop() {
        if (this.cleanupInterval) {
            clearInterval(this.cleanupInterval);
        }

        if (this.wss) {
            this.wss.close();
        }

        this.ledgerStore.flush();

        this.emit('stopped');
    }

    handleConnection(ws, req) {
        const connectionId = randomBytes(16).toString('hex');
        const ip = req.socket.remoteAddress;

        this.stats.totalConnections++;

        this.connections.set(connectionId, {
            ws,
            ip,
            peerId: null,
            connectedAt: Date.now(),
        });

        ws.on('message', (data) => {
            try {
                const message = JSON.parse(data.toString());
                this.handleMessage(connectionId, message);
            } catch (err) {
                console.warn(`[Genesis] Invalid message from ${connectionId}:`, err.message);
            }
        });

        ws.on('close', () => {
            this.handleDisconnect(connectionId);
        });

        ws.on('error', (err) => {
            console.warn(`[Genesis] Connection error ${connectionId}:`, err.message);
        });

        // Send welcome
        this.send(connectionId, {
            type: 'welcome',
            connectionId,
            serverTime: Date.now(),
        });
    }

    handleDisconnect(connectionId) {
        const conn = this.connections.get(connectionId);
        if (conn?.peerId) {
            const peer = this.peerRegistry.get(conn.peerId);
            if (peer?.room) {
                // Notify room peers
                this.broadcastToRoom(peer.room, {
                    type: 'peer-left',
                    peerId: conn.peerId,
                }, conn.peerId);
            }
            this.peerRegistry.remove(conn.peerId);
        }
        this.connections.delete(connectionId);
    }

    handleMessage(connectionId, message) {
        this.stats.totalMessages++;

        const conn = this.connections.get(connectionId);
        if (!conn) return;

        switch (message.type) {
            // Signaling messages
            case 'announce':
                this.handleAnnounce(connectionId, message);
                break;

            case 'join':
                this.handleJoinRoom(connectionId, message);
                break;

            case 'offer':
            case 'answer':
            case 'ice-candidate':
                this.relaySignal(connectionId, message);
                break;

            // Auth messages
            case 'auth-challenge':
                this.handleAuthChallenge(connectionId, message);
                break;

            case 'auth-verify':
                this.handleAuthVerify(connectionId, message);
                break;

            // Ledger messages
            case 'ledger-get':
                this.handleLedgerGet(connectionId, message);
                break;

            case 'ledger-put':
                this.handleLedgerPut(connectionId, message);
                break;

            // DHT bootstrap
            case 'dht-bootstrap':
                this.handleDHTBootstrap(connectionId, message);
                break;

            default:
                console.warn(`[Genesis] Unknown message type: ${message.type}`);
        }
    }

    handleAnnounce(connectionId, message) {
        const conn = this.connections.get(connectionId);
        const peerId = message.piKey || message.peerId || randomBytes(16).toString('hex');

        conn.peerId = peerId;

        this.peerRegistry.register(peerId, {
            publicKey: message.publicKey,
            siteId: message.siteId,
            capabilities: message.capabilities || [],
            connectionId,
        });

        // Send current peer list
        const peers = this.peerRegistry.getAllPeers()
            .filter(p => p.peerId !== peerId)
            .map(p => ({
                piKey: p.peerId,
                siteId: p.siteId,
                capabilities: p.capabilities,
            }));

        this.send(connectionId, {
            type: 'peer-list',
            peers,
        });

        // Notify other peers
        for (const peer of this.peerRegistry.getAllPeers()) {
            if (peer.peerId !== peerId && peer.connectionId) {
                this.send(peer.connectionId, {
                    type: 'peer-joined',
                    peerId,
                    siteId: message.siteId,
                    capabilities: message.capabilities,
                });
            }
        }
    }

    handleJoinRoom(connectionId, message) {
        const conn = this.connections.get(connectionId);
        if (!conn?.peerId) return;

        const room = message.room || 'default';
        this.peerRegistry.joinRoom(conn.peerId, room);

        // Send room peers
        const roomPeers = this.peerRegistry.getRoomPeers(room)
            .filter(p => p.peerId !== conn.peerId)
            .map(p => ({
                piKey: p.peerId,
                siteId: p.siteId,
            }));

        this.send(connectionId, {
            type: 'room-joined',
            room,
            peers: roomPeers,
        });

        // Notify room peers
        this.broadcastToRoom(room, {
            type: 'peer-joined',
            peerId: conn.peerId,
            siteId: this.peerRegistry.get(conn.peerId)?.siteId,
        }, conn.peerId);
    }

    relaySignal(connectionId, message) {
        this.stats.signalsRelayed++;

        const conn = this.connections.get(connectionId);
        if (!conn?.peerId) return;

        const targetPeer = this.peerRegistry.get(message.to);
        if (!targetPeer?.connectionId) {
            this.send(connectionId, {
                type: 'error',
                error: 'Target peer not found',
                originalType: message.type,
            });
            return;
        }

        // Relay the signal
        this.send(targetPeer.connectionId, {
            ...message,
            from: conn.peerId,
        });
    }

    handleAuthChallenge(connectionId, message) {
        const { nonce, challenge } = this.authService.createChallenge(
            message.publicKey,
            message.deviceId
        );

        this.send(connectionId, {
            type: 'auth-challenge-response',
            nonce,
            challenge,
        });
    }

    handleAuthVerify(connectionId, message) {
        const result = this.authService.verifyChallenge(
            message.nonce,
            message.publicKey,
            message.signature
        );

        this.send(connectionId, {
            type: 'auth-verify-response',
            ...result,
        });
    }

    handleLedgerGet(connectionId, message) {
        const tokenData = this.authService.validateToken(message.token);
        if (!tokenData) {
            this.send(connectionId, {
                type: 'ledger-response',
                error: 'Invalid or expired token',
            });
            return;
        }

        const states = this.ledgerStore.getStates(message.publicKey || tokenData.publicKey);

        this.send(connectionId, {
            type: 'ledger-response',
            states,
        });
    }

    handleLedgerPut(connectionId, message) {
        const tokenData = this.authService.validateToken(message.token);
        if (!tokenData) {
            this.send(connectionId, {
                type: 'ledger-put-response',
                error: 'Invalid or expired token',
            });
            return;
        }

        const updated = this.ledgerStore.update(
            tokenData.publicKey,
            message.deviceId || tokenData.deviceId,
            message.state
        );

        this.send(connectionId, {
            type: 'ledger-put-response',
            success: true,
            state: updated,
        });
    }

    handleDHTBootstrap(connectionId, message) {
        // Return known peers for DHT bootstrap
        const peers = this.peerRegistry.getAllPeers()
            .slice(0, 20)
            .map(p => ({
                id: p.peerId,
                address: p.connectionId,
                lastSeen: p.lastSeen,
            }));

        this.send(connectionId, {
            type: 'dht-bootstrap-response',
            peers,
        });
    }

    send(connectionId, message) {
        const conn = this.connections.get(connectionId);
        if (conn?.ws?.readyState === 1) {
            conn.ws.send(JSON.stringify(message));
        }
    }

    broadcastToRoom(room, message, excludePeerId = null) {
        const peers = this.peerRegistry.getRoomPeers(room);
        for (const peer of peers) {
            if (peer.peerId !== excludePeerId && peer.connectionId) {
                this.send(peer.connectionId, message);
            }
        }
    }

    cleanup() {
        // Prune stale peers
        const removed = this.peerRegistry.pruneStale();
        if (removed.length > 0) {
            console.log(`[Genesis] Pruned ${removed.length} stale peers`);
        }

        // Cleanup auth
        this.authService.cleanup();
    }

    getStats() {
        return {
            ...this.stats,
            uptime: this.stats.startedAt ? Date.now() - this.stats.startedAt : 0,
            ...this.peerRegistry.getStats(),
            ...this.ledgerStore.getStats(),
            activeConnections: this.connections.size,
        };
    }
}

// ============================================
// CLI
// ============================================

async function main() {
    const args = process.argv.slice(2);

    // Parse args
    let port = GENESIS_CONFIG.port;
    let dataDir = GENESIS_CONFIG.dataDir;

    for (let i = 0; i < args.length; i++) {
        if (args[i] === '--port' && args[i + 1]) {
            port = parseInt(args[i + 1]);
            i++;
        } else if (args[i] === '--data' && args[i + 1]) {
            dataDir = args[i + 1];
            i++;
        } else if (args[i] === '--help') {
            console.log(`
Edge-Net Genesis Node

Usage: node genesis.js [options]

Options:
  --port <port>    Port to listen on (default: 8787)
  --data <dir>     Data directory (default: ~/.ruvector/genesis)
  --help           Show this help

Environment Variables:
  GENESIS_PORT     Port (default: 8787)
  GENESIS_HOST     Host (default: 0.0.0.0)
  GENESIS_DATA     Data directory

Examples:
  node genesis.js
  node genesis.js --port 9000
  node genesis.js --port 8787 --data /var/lib/edge-net
`);
            process.exit(0);
        }
    }

    const genesis = new GenesisNode({ port, dataDir });

    // Handle shutdown
    process.on('SIGINT', () => {
        console.log('\n\n🛑 Shutting down Genesis Node...');
        genesis.stop();
        process.exit(0);
    });

    process.on('SIGTERM', () => {
        genesis.stop();
        process.exit(0);
    });

    // Start server
    await genesis.start();

    // Log stats periodically
    setInterval(() => {
        const stats = genesis.getStats();
        console.log(`[Genesis] Peers: ${stats.totalPeers} | Connections: ${stats.activeConnections} | Signals: ${stats.signalsRelayed}`);
    }, 60000);
}

// Run if executed directly
if (process.argv[1]?.endsWith('genesis.js')) {
    main().catch(err => {
        console.error('Genesis Node error:', err);
        process.exit(1);
    });
}

export default GenesisNode;
