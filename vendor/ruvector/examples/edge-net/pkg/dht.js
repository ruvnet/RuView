/**
 * @ruvector/edge-net DHT (Distributed Hash Table)
 *
 * Kademlia-style DHT for decentralized peer discovery.
 * Works without central signaling servers.
 *
 * Features:
 * - XOR distance-based routing
 * - K-bucket peer organization
 * - Iterative node lookup
 * - Value storage and retrieval
 * - Peer discovery protocol
 *
 * @module @ruvector/edge-net/dht
 */

import { EventEmitter } from 'events';
import { createHash, randomBytes } from 'crypto';

// DHT Constants
const K = 20;           // K-bucket size (max peers per bucket)
const ALPHA = 3;        // Parallel lookup concurrency
const ID_BITS = 160;    // SHA-1 hash bits
const REFRESH_INTERVAL = 60000;
const PEER_TIMEOUT = 300000; // 5 minutes

/**
 * Calculate XOR distance between two node IDs
 */
export function xorDistance(id1, id2) {
    const buf1 = Buffer.from(id1, 'hex');
    const buf2 = Buffer.from(id2, 'hex');
    const result = Buffer.alloc(Math.max(buf1.length, buf2.length));

    for (let i = 0; i < result.length; i++) {
        result[i] = (buf1[i] || 0) ^ (buf2[i] || 0);
    }

    return result.toString('hex');
}

/**
 * Get the bucket index for a given distance
 */
export function getBucketIndex(distance) {
    const buf = Buffer.from(distance, 'hex');

    for (let i = 0; i < buf.length; i++) {
        if (buf[i] !== 0) {
            // Find the first set bit
            for (let j = 7; j >= 0; j--) {
                if (buf[i] & (1 << j)) {
                    return (buf.length - i - 1) * 8 + j;
                }
            }
        }
    }

    return 0;
}

/**
 * Generate a random node ID
 */
export function generateNodeId() {
    return createHash('sha1').update(randomBytes(32)).digest('hex');
}

/**
 * K-Bucket: Stores peers at similar XOR distance
 */
export class KBucket {
    constructor(index, k = K) {
        this.index = index;
        this.k = k;
        this.peers = [];
        this.replacementCache = [];
    }

    /**
     * Add a peer to the bucket
     */
    add(peer) {
        // Check if peer already exists
        const existingIndex = this.peers.findIndex(p => p.id === peer.id);

        if (existingIndex !== -1) {
            // Move to end (most recently seen)
            this.peers.splice(existingIndex, 1);
            this.peers.push({ ...peer, lastSeen: Date.now() });
            return true;
        }

        if (this.peers.length < this.k) {
            this.peers.push({ ...peer, lastSeen: Date.now() });
            return true;
        }

        // Bucket full, add to replacement cache
        this.replacementCache.push({ ...peer, lastSeen: Date.now() });
        if (this.replacementCache.length > this.k) {
            this.replacementCache.shift();
        }

        return false;
    }

    /**
     * Remove a peer from the bucket
     */
    remove(peerId) {
        const index = this.peers.findIndex(p => p.id === peerId);
        if (index !== -1) {
            this.peers.splice(index, 1);

            // Promote from replacement cache
            if (this.replacementCache.length > 0) {
                this.peers.push(this.replacementCache.shift());
            }

            return true;
        }
        return false;
    }

    /**
     * Get a peer by ID
     */
    get(peerId) {
        return this.peers.find(p => p.id === peerId);
    }

    /**
     * Get all peers
     */
    getAll() {
        return [...this.peers];
    }

    /**
     * Get closest peers to a target ID
     */
    getClosest(targetId, count = K) {
        return this.peers
            .map(p => ({
                ...p,
                distance: xorDistance(p.id, targetId),
            }))
            .sort((a, b) => a.distance.localeCompare(b.distance))
            .slice(0, count);
    }

    /**
     * Remove stale peers
     */
    prune() {
        const now = Date.now();
        this.peers = this.peers.filter(p =>
            now - p.lastSeen < PEER_TIMEOUT
        );
    }

    get size() {
        return this.peers.length;
    }
}

/**
 * Routing Table: Manages all K-buckets
 */
export class RoutingTable {
    constructor(localId) {
        this.localId = localId;
        this.buckets = new Array(ID_BITS).fill(null).map((_, i) => new KBucket(i));
        this.allPeers = new Map();
    }

    /**
     * Add a peer to the routing table
     */
    add(peer) {
        if (peer.id === this.localId) return false;

        const distance = xorDistance(this.localId, peer.id);
        const bucketIndex = getBucketIndex(distance);
        const added = this.buckets[bucketIndex].add(peer);

        if (added) {
            this.allPeers.set(peer.id, peer);
        }

        return added;
    }

    /**
     * Remove a peer from the routing table
     */
    remove(peerId) {
        const peer = this.allPeers.get(peerId);
        if (!peer) return false;

        const distance = xorDistance(this.localId, peerId);
        const bucketIndex = getBucketIndex(distance);
        this.buckets[bucketIndex].remove(peerId);
        this.allPeers.delete(peerId);

        return true;
    }

    /**
     * Get a peer by ID
     */
    get(peerId) {
        return this.allPeers.get(peerId);
    }

    /**
     * Find the closest peers to a target ID
     */
    findClosest(targetId, count = K) {
        const candidates = [];

        for (const bucket of this.buckets) {
            candidates.push(...bucket.getAll());
        }

        return candidates
            .map(p => ({
                ...p,
                distance: xorDistance(p.id, targetId),
            }))
            .sort((a, b) => a.distance.localeCompare(b.distance))
            .slice(0, count);
    }

    /**
     * Get all peers
     */
    getAllPeers() {
        return Array.from(this.allPeers.values());
    }

    /**
     * Prune stale peers from all buckets
     */
    prune() {
        for (const bucket of this.buckets) {
            bucket.prune();
        }

        // Update allPeers map
        this.allPeers.clear();
        for (const bucket of this.buckets) {
            for (const peer of bucket.getAll()) {
                this.allPeers.set(peer.id, peer);
            }
        }
    }

    /**
     * Get routing table stats
     */
    getStats() {
        let totalPeers = 0;
        let bucketsUsed = 0;

        for (const bucket of this.buckets) {
            if (bucket.size > 0) {
                totalPeers += bucket.size;
                bucketsUsed++;
            }
        }

        return {
            totalPeers,
            bucketsUsed,
            bucketCount: this.buckets.length,
        };
    }
}

/**
 * DHT Node: Full DHT implementation
 */
export class DHTNode extends EventEmitter {
    constructor(options = {}) {
        super();
        this.id = options.id || generateNodeId();
        this.routingTable = new RoutingTable(this.id);
        this.storage = new Map(); // DHT value storage
        this.pendingLookups = new Map();
        this.transport = options.transport || null;
        this.bootstrapNodes = options.bootstrapNodes || [];

        this.stats = {
            lookups: 0,
            stores: 0,
            finds: 0,
            messagesReceived: 0,
            messagesSent: 0,
        };

        // Refresh timer
        this.refreshTimer = null;
    }

    /**
     * Start the DHT node
     */
    async start() {
        console.log(`\n🌐 Starting DHT Node: ${this.id.slice(0, 8)}...`);

        // Bootstrap from known nodes
        if (this.bootstrapNodes.length > 0) {
            await this.bootstrap();
        }

        // Start periodic refresh
        this.refreshTimer = setInterval(() => {
            this.refresh();
        }, REFRESH_INTERVAL);

        this.emit('started', { id: this.id });

        return this;
    }

    /**
     * Stop the DHT node
     */
    stop() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
        }
        this.emit('stopped');
    }

    /**
     * Bootstrap from known nodes
     */
    async bootstrap() {
        console.log(`  📡 Bootstrapping from ${this.bootstrapNodes.length} nodes...`);

        for (const node of this.bootstrapNodes) {
            try {
                // Add bootstrap node to routing table
                this.routingTable.add({
                    id: node.id,
                    address: node.address,
                    port: node.port,
                });

                // Perform lookup for our own ID to populate routing table
                await this.lookup(this.id);
            } catch (err) {
                console.warn(`  ⚠️ Bootstrap node ${node.id.slice(0, 8)} unreachable`);
            }
        }
    }

    /**
     * Add a peer to the routing table
     */
    addPeer(peer) {
        const added = this.routingTable.add(peer);
        if (added) {
            this.emit('peer-added', peer);
        }
        return added;
    }

    /**
     * Remove a peer from the routing table
     */
    removePeer(peerId) {
        const removed = this.routingTable.remove(peerId);
        if (removed) {
            this.emit('peer-removed', peerId);
        }
        return removed;
    }

    /**
     * Iterative node lookup (Kademlia FIND_NODE)
     */
    async lookup(targetId) {
        this.stats.lookups++;

        // Get initial closest nodes
        let closest = this.routingTable.findClosest(targetId, ALPHA);
        const queried = new Set([this.id]);
        const results = new Map();

        // Add initial closest to results
        for (const node of closest) {
            results.set(node.id, node);
        }

        // Iterative lookup
        while (closest.length > 0) {
            const toQuery = closest.filter(n => !queried.has(n.id)).slice(0, ALPHA);

            if (toQuery.length === 0) break;

            // Query nodes in parallel
            const responses = await Promise.all(
                toQuery.map(async (node) => {
                    queried.add(node.id);
                    try {
                        return await this.sendFindNode(node, targetId);
                    } catch (err) {
                        return [];
                    }
                })
            );

            // Process responses
            let foundCloser = false;
            for (const nodes of responses) {
                for (const node of nodes) {
                    if (!results.has(node.id)) {
                        results.set(node.id, node);
                        this.routingTable.add(node);
                        foundCloser = true;
                    }
                }
            }

            if (!foundCloser) break;

            // Get new closest
            closest = Array.from(results.values())
                .filter(n => !queried.has(n.id))
                .sort((a, b) => {
                    const distA = xorDistance(a.id, targetId);
                    const distB = xorDistance(b.id, targetId);
                    return distA.localeCompare(distB);
                })
                .slice(0, K);
        }

        return Array.from(results.values())
            .sort((a, b) => {
                const distA = xorDistance(a.id, targetId);
                const distB = xorDistance(b.id, targetId);
                return distA.localeCompare(distB);
            })
            .slice(0, K);
    }

    /**
     * Store a value in the DHT
     */
    async store(key, value) {
        this.stats.stores++;

        const keyHash = createHash('sha1').update(key).digest('hex');

        // Store locally
        this.storage.set(keyHash, {
            key,
            value,
            timestamp: Date.now(),
        });

        // Find closest nodes to the key
        const closest = await this.lookup(keyHash);

        // Store on closest nodes
        await Promise.all(
            closest.map(node => this.sendStore(node, keyHash, value))
        );

        this.emit('stored', { key, keyHash });

        return keyHash;
    }

    /**
     * Find a value in the DHT
     */
    async find(key) {
        this.stats.finds++;

        const keyHash = createHash('sha1').update(key).digest('hex');

        // Check local storage first
        const local = this.storage.get(keyHash);
        if (local) {
            return local.value;
        }

        // Query closest nodes
        const closest = await this.lookup(keyHash);

        for (const node of closest) {
            try {
                const value = await this.sendFindValue(node, keyHash);
                if (value) {
                    // Cache locally
                    this.storage.set(keyHash, {
                        key,
                        value,
                        timestamp: Date.now(),
                    });
                    return value;
                }
            } catch (err) {
                // Node didn't have value
            }
        }

        return null;
    }

    /**
     * Send FIND_NODE request
     */
    async sendFindNode(node, targetId) {
        this.stats.messagesSent++;

        if (this.transport) {
            return await this.transport.send(node, {
                type: 'FIND_NODE',
                sender: this.id,
                target: targetId,
            });
        }

        // Simulated response for local testing
        return [];
    }

    /**
     * Send STORE request
     */
    async sendStore(node, keyHash, value) {
        this.stats.messagesSent++;

        if (this.transport) {
            return await this.transport.send(node, {
                type: 'STORE',
                sender: this.id,
                key: keyHash,
                value,
            });
        }
    }

    /**
     * Send FIND_VALUE request
     */
    async sendFindValue(node, keyHash) {
        this.stats.messagesSent++;

        if (this.transport) {
            const response = await this.transport.send(node, {
                type: 'FIND_VALUE',
                sender: this.id,
                key: keyHash,
            });
            return response?.value;
        }

        return null;
    }

    /**
     * Handle incoming DHT message
     */
    async handleMessage(message, sender) {
        this.stats.messagesReceived++;

        // Add sender to routing table
        this.routingTable.add(sender);

        switch (message.type) {
            case 'PING':
                return { type: 'PONG', sender: this.id };

            case 'FIND_NODE':
                return {
                    type: 'FIND_NODE_RESPONSE',
                    sender: this.id,
                    nodes: this.routingTable.findClosest(message.target, K),
                };

            case 'STORE':
                this.storage.set(message.key, {
                    value: message.value,
                    timestamp: Date.now(),
                });
                return { type: 'STORE_ACK', sender: this.id };

            case 'FIND_VALUE':
                const stored = this.storage.get(message.key);
                if (stored) {
                    return {
                        type: 'FIND_VALUE_RESPONSE',
                        sender: this.id,
                        value: stored.value,
                    };
                }
                return {
                    type: 'FIND_VALUE_RESPONSE',
                    sender: this.id,
                    nodes: this.routingTable.findClosest(message.key, K),
                };

            default:
                return null;
        }
    }

    /**
     * Refresh buckets by looking up random IDs
     */
    refresh() {
        this.routingTable.prune();

        // Lookup random ID in each bucket that hasn't been updated recently
        for (let i = 0; i < ID_BITS; i++) {
            const bucket = this.routingTable.buckets[i];
            if (bucket.size > 0) {
                const randomTarget = generateNodeId();
                this.lookup(randomTarget).catch(() => {});
            }
        }
    }

    /**
     * Get DHT statistics
     */
    getStats() {
        return {
            ...this.stats,
            ...this.routingTable.getStats(),
            storageSize: this.storage.size,
        };
    }

    /**
     * Get all known peers
     */
    getPeers() {
        return this.routingTable.getAllPeers();
    }

    /**
     * Find peers providing a service
     */
    async findProviders(service) {
        const serviceKey = `service:${service}`;
        return await this.find(serviceKey);
    }

    /**
     * Announce as a provider of a service
     */
    async announce(service) {
        const serviceKey = `service:${service}`;

        // Get existing providers
        let providers = await this.find(serviceKey);
        if (!providers) {
            providers = [];
        }

        // Add ourselves
        if (!providers.some(p => p.id === this.id)) {
            providers.push({
                id: this.id,
                timestamp: Date.now(),
            });
        }

        // Store updated providers list
        await this.store(serviceKey, providers);

        this.emit('announced', { service });
    }
}

/**
 * WebRTC Transport for DHT
 */
export class DHTWebRTCTransport extends EventEmitter {
    constructor(peerManager) {
        super();
        this.peerManager = peerManager;
        this.pendingRequests = new Map();
        this.requestId = 0;

        // Listen for DHT messages from peers
        this.peerManager.on('message', ({ from, message }) => {
            if (message.type?.startsWith('DHT_')) {
                this.handleResponse(from, message);
            }
        });
    }

    /**
     * Send DHT message to a peer
     */
    async send(node, message) {
        return new Promise((resolve, reject) => {
            const requestId = ++this.requestId;

            // Set timeout
            const timeout = setTimeout(() => {
                this.pendingRequests.delete(requestId);
                reject(new Error('DHT request timeout'));
            }, 10000);

            this.pendingRequests.set(requestId, { resolve, reject, timeout });

            // Send via WebRTC
            const sent = this.peerManager.sendToPeer(node.id, {
                ...message,
                type: `DHT_${message.type}`,
                requestId,
            });

            if (!sent) {
                clearTimeout(timeout);
                this.pendingRequests.delete(requestId);
                reject(new Error('Peer not connected'));
            }
        });
    }

    /**
     * Handle DHT response
     */
    handleResponse(from, message) {
        const pending = this.pendingRequests.get(message.requestId);
        if (pending) {
            clearTimeout(pending.timeout);
            this.pendingRequests.delete(message.requestId);
            pending.resolve(message);
        }
    }
}

/**
 * Create and configure a DHT node with WebRTC transport
 */
export async function createDHTNode(peerManager, options = {}) {
    const transport = new DHTWebRTCTransport(peerManager);

    const dht = new DHTNode({
        ...options,
        transport,
    });

    // Forward DHT messages from peers
    peerManager.on('message', ({ from, message }) => {
        if (message.type?.startsWith('DHT_')) {
            const dhtMessage = {
                ...message,
                type: message.type.replace('DHT_', ''),
            };

            const sender = {
                id: from,
                lastSeen: Date.now(),
            };

            dht.handleMessage(dhtMessage, sender).then(response => {
                if (response) {
                    peerManager.sendToPeer(from, {
                        ...response,
                        type: `DHT_${response.type}`,
                        requestId: message.requestId,
                    });
                }
            });
        }
    });

    await dht.start();

    return dht;
}

// ============================================
// EXPORTS
// ============================================

export default DHTNode;
