/**
 * @ruvector/edge-net WebRTC Signaling Server
 *
 * Real signaling server for WebRTC peer connections
 * Enables true P2P connections between nodes
 *
 * @module @ruvector/edge-net/signaling
 */

import { EventEmitter } from 'events';
import { createServer } from 'http';
import { randomBytes, createHash } from 'crypto';

// ============================================
// SIGNALING SERVER
// ============================================

/**
 * WebRTC Signaling Server
 * Routes offers, answers, and ICE candidates between peers
 */
export class SignalingServer extends EventEmitter {
    constructor(options = {}) {
        super();
        this.port = options.port || 8765;
        this.server = null;
        this.wss = null;

        this.peers = new Map();      // peerId -> { ws, info, rooms }
        this.rooms = new Map();      // roomId -> Set<peerId>
        this.pendingOffers = new Map(); // offerId -> { from, to, offer }

        this.stats = {
            connections: 0,
            messages: 0,
            offers: 0,
            answers: 0,
            iceCandidates: 0,
        };
    }

    /**
     * Start the signaling server
     */
    async start() {
        return new Promise(async (resolve, reject) => {
            try {
                // Create HTTP server
                this.server = createServer((req, res) => {
                    if (req.url === '/health') {
                        res.writeHead(200, { 'Content-Type': 'application/json' });
                        res.end(JSON.stringify({ status: 'ok', peers: this.peers.size }));
                    } else if (req.url === '/stats') {
                        res.writeHead(200, { 'Content-Type': 'application/json' });
                        res.end(JSON.stringify(this.getStats()));
                    } else {
                        res.writeHead(404);
                        res.end('Not found');
                    }
                });

                // Create WebSocket server
                const { WebSocketServer } = await import('ws');
                this.wss = new WebSocketServer({ server: this.server });

                this.wss.on('connection', (ws, req) => {
                    this.handleConnection(ws, req);
                });

                this.server.listen(this.port, () => {
                    console.log(`[Signaling] Server running on port ${this.port}`);
                    this.emit('ready', { port: this.port });
                    resolve(this);
                });

            } catch (error) {
                reject(error);
            }
        });
    }

    /**
     * Handle new WebSocket connection
     */
    handleConnection(ws, req) {
        const peerId = `peer-${randomBytes(8).toString('hex')}`;

        const peerInfo = {
            id: peerId,
            ws,
            info: {},
            rooms: new Set(),
            connectedAt: Date.now(),
            lastSeen: Date.now(),
        };

        this.peers.set(peerId, peerInfo);
        this.stats.connections++;

        // Send welcome message
        this.sendTo(peerId, {
            type: 'welcome',
            peerId,
            serverTime: Date.now(),
        });

        ws.on('message', (data) => {
            try {
                const message = JSON.parse(data.toString());
                this.handleMessage(peerId, message);
            } catch (error) {
                console.error('[Signaling] Invalid message:', error.message);
            }
        });

        ws.on('close', () => {
            this.handleDisconnect(peerId);
        });

        ws.on('error', (error) => {
            console.error(`[Signaling] Peer ${peerId} error:`, error.message);
        });

        this.emit('peer-connected', { peerId });
    }

    /**
     * Handle incoming message from peer
     */
    handleMessage(peerId, message) {
        const peer = this.peers.get(peerId);
        if (!peer) return;

        peer.lastSeen = Date.now();
        this.stats.messages++;

        switch (message.type) {
            case 'register':
                this.handleRegister(peerId, message);
                break;

            case 'join-room':
                this.handleJoinRoom(peerId, message);
                break;

            case 'leave-room':
                this.handleLeaveRoom(peerId, message);
                break;

            case 'offer':
                this.handleOffer(peerId, message);
                break;

            case 'answer':
                this.handleAnswer(peerId, message);
                break;

            case 'ice-candidate':
                this.handleIceCandidate(peerId, message);
                break;

            case 'discover':
                this.handleDiscover(peerId, message);
                break;

            case 'broadcast':
                this.handleBroadcast(peerId, message);
                break;

            case 'ping':
                this.sendTo(peerId, { type: 'pong', timestamp: Date.now() });
                break;

            default:
                console.log(`[Signaling] Unknown message type: ${message.type}`);
        }
    }

    /**
     * Handle peer registration
     */
    handleRegister(peerId, message) {
        const peer = this.peers.get(peerId);
        if (!peer) return;

        peer.info = {
            nodeId: message.nodeId,
            capabilities: message.capabilities || [],
            publicKey: message.publicKey,
            region: message.region,
        };

        this.sendTo(peerId, {
            type: 'registered',
            peerId,
            info: peer.info,
        });

        this.emit('peer-registered', { peerId, info: peer.info });
    }

    /**
     * Handle room join
     */
    handleJoinRoom(peerId, message) {
        const roomId = message.roomId || 'default';
        const peer = this.peers.get(peerId);
        if (!peer) return;

        // Create room if doesn't exist
        if (!this.rooms.has(roomId)) {
            this.rooms.set(roomId, new Set());
        }

        const room = this.rooms.get(roomId);
        room.add(peerId);
        peer.rooms.add(roomId);

        // Get existing peers in room
        const existingPeers = Array.from(room)
            .filter(id => id !== peerId)
            .map(id => {
                const p = this.peers.get(id);
                return { peerId: id, info: p?.info };
            });

        // Notify joining peer of existing peers
        this.sendTo(peerId, {
            type: 'room-joined',
            roomId,
            peers: existingPeers,
        });

        // Notify existing peers of new peer
        for (const otherPeerId of room) {
            if (otherPeerId !== peerId) {
                this.sendTo(otherPeerId, {
                    type: 'peer-joined',
                    roomId,
                    peerId,
                    info: peer.info,
                });
            }
        }

        this.emit('room-join', { roomId, peerId });
    }

    /**
     * Handle room leave
     */
    handleLeaveRoom(peerId, message) {
        const roomId = message.roomId;
        const peer = this.peers.get(peerId);
        if (!peer) return;

        const room = this.rooms.get(roomId);
        if (!room) return;

        room.delete(peerId);
        peer.rooms.delete(roomId);

        // Notify other peers
        for (const otherPeerId of room) {
            this.sendTo(otherPeerId, {
                type: 'peer-left',
                roomId,
                peerId,
            });
        }

        // Clean up empty room
        if (room.size === 0) {
            this.rooms.delete(roomId);
        }
    }

    /**
     * Handle WebRTC offer
     */
    handleOffer(peerId, message) {
        this.stats.offers++;

        const targetPeerId = message.to;
        const target = this.peers.get(targetPeerId);

        if (!target) {
            this.sendTo(peerId, {
                type: 'error',
                error: 'Peer not found',
                targetPeerId,
            });
            return;
        }

        // Forward offer to target
        this.sendTo(targetPeerId, {
            type: 'offer',
            from: peerId,
            offer: message.offer,
            connectionId: message.connectionId,
        });

        this.emit('offer', { from: peerId, to: targetPeerId });
    }

    /**
     * Handle WebRTC answer
     */
    handleAnswer(peerId, message) {
        this.stats.answers++;

        const targetPeerId = message.to;
        const target = this.peers.get(targetPeerId);

        if (!target) return;

        // Forward answer to target
        this.sendTo(targetPeerId, {
            type: 'answer',
            from: peerId,
            answer: message.answer,
            connectionId: message.connectionId,
        });

        this.emit('answer', { from: peerId, to: targetPeerId });
    }

    /**
     * Handle ICE candidate
     */
    handleIceCandidate(peerId, message) {
        this.stats.iceCandidates++;

        const targetPeerId = message.to;
        const target = this.peers.get(targetPeerId);

        if (!target) return;

        // Forward ICE candidate to target
        this.sendTo(targetPeerId, {
            type: 'ice-candidate',
            from: peerId,
            candidate: message.candidate,
            connectionId: message.connectionId,
        });
    }

    /**
     * Handle peer discovery request
     */
    handleDiscover(peerId, message) {
        const capabilities = message.capabilities || [];
        const limit = message.limit || 10;

        const matches = [];

        for (const [id, peer] of this.peers) {
            if (id === peerId) continue;

            // Check capability match
            if (capabilities.length > 0) {
                const peerCaps = peer.info.capabilities || [];
                const hasMatch = capabilities.some(cap => peerCaps.includes(cap));
                if (!hasMatch) continue;
            }

            matches.push({
                peerId: id,
                info: peer.info,
                lastSeen: peer.lastSeen,
            });

            if (matches.length >= limit) break;
        }

        this.sendTo(peerId, {
            type: 'discover-result',
            peers: matches,
            total: this.peers.size - 1,
        });
    }

    /**
     * Handle broadcast to room
     */
    handleBroadcast(peerId, message) {
        const roomId = message.roomId;
        const room = this.rooms.get(roomId);

        if (!room) return;

        for (const otherPeerId of room) {
            if (otherPeerId !== peerId) {
                this.sendTo(otherPeerId, {
                    type: 'broadcast',
                    from: peerId,
                    roomId,
                    data: message.data,
                });
            }
        }
    }

    /**
     * Handle peer disconnect
     */
    handleDisconnect(peerId) {
        const peer = this.peers.get(peerId);
        if (!peer) return;

        // Leave all rooms
        for (const roomId of peer.rooms) {
            const room = this.rooms.get(roomId);
            if (room) {
                room.delete(peerId);

                // Notify other peers
                for (const otherPeerId of room) {
                    this.sendTo(otherPeerId, {
                        type: 'peer-left',
                        roomId,
                        peerId,
                    });
                }

                // Clean up empty room
                if (room.size === 0) {
                    this.rooms.delete(roomId);
                }
            }
        }

        this.peers.delete(peerId);
        this.emit('peer-disconnected', { peerId });
    }

    /**
     * Send message to peer
     */
    sendTo(peerId, message) {
        const peer = this.peers.get(peerId);
        if (peer && peer.ws.readyState === 1) {
            peer.ws.send(JSON.stringify(message));
            return true;
        }
        return false;
    }

    /**
     * Get server stats
     */
    getStats() {
        return {
            peers: this.peers.size,
            rooms: this.rooms.size,
            ...this.stats,
            uptime: Date.now() - (this.startTime || Date.now()),
        };
    }

    /**
     * Stop the server
     */
    async stop() {
        return new Promise((resolve) => {
            // Close all peer connections
            for (const [peerId, peer] of this.peers) {
                peer.ws.close();
            }

            this.peers.clear();
            this.rooms.clear();

            if (this.wss) {
                this.wss.close();
            }

            if (this.server) {
                this.server.close(() => {
                    console.log('[Signaling] Server stopped');
                    resolve();
                });
            } else {
                resolve();
            }
        });
    }
}

// ============================================
// SIGNALING CLIENT
// ============================================

/**
 * WebRTC Signaling Client
 * Connects to signaling server for peer discovery and connection setup
 */
export class SignalingClient extends EventEmitter {
    constructor(options = {}) {
        super();
        this.serverUrl = options.serverUrl || 'ws://localhost:8765';
        this.nodeId = options.nodeId || `node-${randomBytes(8).toString('hex')}`;
        this.capabilities = options.capabilities || [];

        this.ws = null;
        this.peerId = null;
        this.connected = false;
        this.rooms = new Set();

        this.pendingConnections = new Map();
        this.peerConnections = new Map();
    }

    /**
     * Connect to signaling server
     */
    async connect() {
        return new Promise(async (resolve, reject) => {
            try {
                let WebSocket;
                if (typeof globalThis.WebSocket !== 'undefined') {
                    WebSocket = globalThis.WebSocket;
                } else {
                    const ws = await import('ws');
                    WebSocket = ws.default || ws.WebSocket;
                }

                this.ws = new WebSocket(this.serverUrl);

                const timeout = setTimeout(() => {
                    reject(new Error('Connection timeout'));
                }, 10000);

                this.ws.onopen = () => {
                    clearTimeout(timeout);
                    this.connected = true;
                    this.emit('connected');
                };

                this.ws.onmessage = (event) => {
                    const message = JSON.parse(event.data);
                    this.handleMessage(message);

                    if (message.type === 'registered') {
                        resolve(this);
                    }
                };

                this.ws.onclose = () => {
                    this.connected = false;
                    this.emit('disconnected');
                };

                this.ws.onerror = (error) => {
                    clearTimeout(timeout);
                    reject(error);
                };

            } catch (error) {
                reject(error);
            }
        });
    }

    /**
     * Handle incoming message
     */
    handleMessage(message) {
        switch (message.type) {
            case 'welcome':
                this.peerId = message.peerId;
                // Register with capabilities
                this.send({
                    type: 'register',
                    nodeId: this.nodeId,
                    capabilities: this.capabilities,
                });
                break;

            case 'registered':
                this.emit('registered', message);
                break;

            case 'room-joined':
                this.rooms.add(message.roomId);
                this.emit('room-joined', message);
                break;

            case 'peer-joined':
                this.emit('peer-joined', message);
                break;

            case 'peer-left':
                this.emit('peer-left', message);
                break;

            case 'offer':
                this.emit('offer', message);
                break;

            case 'answer':
                this.emit('answer', message);
                break;

            case 'ice-candidate':
                this.emit('ice-candidate', message);
                break;

            case 'discover-result':
                this.emit('discover-result', message);
                break;

            case 'broadcast':
                this.emit('broadcast', message);
                break;

            case 'pong':
                this.emit('pong', message);
                break;

            default:
                this.emit('message', message);
        }
    }

    /**
     * Send message to server
     */
    send(message) {
        if (this.connected && this.ws?.readyState === 1) {
            this.ws.send(JSON.stringify(message));
            return true;
        }
        return false;
    }

    /**
     * Join a room
     */
    joinRoom(roomId) {
        return this.send({ type: 'join-room', roomId });
    }

    /**
     * Leave a room
     */
    leaveRoom(roomId) {
        this.rooms.delete(roomId);
        return this.send({ type: 'leave-room', roomId });
    }

    /**
     * Send WebRTC offer to peer
     */
    sendOffer(targetPeerId, offer, connectionId) {
        return this.send({
            type: 'offer',
            to: targetPeerId,
            offer,
            connectionId,
        });
    }

    /**
     * Send WebRTC answer to peer
     */
    sendAnswer(targetPeerId, answer, connectionId) {
        return this.send({
            type: 'answer',
            to: targetPeerId,
            answer,
            connectionId,
        });
    }

    /**
     * Send ICE candidate to peer
     */
    sendIceCandidate(targetPeerId, candidate, connectionId) {
        return this.send({
            type: 'ice-candidate',
            to: targetPeerId,
            candidate,
            connectionId,
        });
    }

    /**
     * Discover peers with capabilities
     */
    discover(capabilities = [], limit = 10) {
        return this.send({
            type: 'discover',
            capabilities,
            limit,
        });
    }

    /**
     * Broadcast to room
     */
    broadcast(roomId, data) {
        return this.send({
            type: 'broadcast',
            roomId,
            data,
        });
    }

    /**
     * Ping server
     */
    ping() {
        return this.send({ type: 'ping' });
    }

    /**
     * Close connection
     */
    close() {
        if (this.ws) {
            this.ws.close();
        }
    }
}

// ============================================
// EXPORTS
// ============================================

export default SignalingServer;
