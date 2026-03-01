/**
 * @ruvector/edge-net Persistent Ledger with CRDT
 *
 * Conflict-free Replicated Data Type for distributed credit tracking
 * Features:
 * - G-Counter for earned credits
 * - PN-Counter for balance
 * - LWW-Register for metadata
 * - File-based persistence
 * - Network synchronization
 *
 * @module @ruvector/edge-net/ledger
 */

import { EventEmitter } from 'events';
import { randomBytes, createHash } from 'crypto';
import { existsSync, mkdirSync, writeFileSync, readFileSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';

// ============================================
// CRDT PRIMITIVES
// ============================================

/**
 * G-Counter (Grow-only Counter)
 * Can only increment, never decrement
 */
export class GCounter {
    constructor(nodeId) {
        this.nodeId = nodeId;
        this.counters = new Map(); // nodeId -> count
    }

    increment(amount = 1) {
        const current = this.counters.get(this.nodeId) || 0;
        this.counters.set(this.nodeId, current + amount);
    }

    value() {
        let total = 0;
        for (const count of this.counters.values()) {
            total += count;
        }
        return total;
    }

    merge(other) {
        for (const [nodeId, count] of other.counters) {
            const current = this.counters.get(nodeId) || 0;
            this.counters.set(nodeId, Math.max(current, count));
        }
    }

    toJSON() {
        return {
            nodeId: this.nodeId,
            counters: Object.fromEntries(this.counters),
        };
    }

    static fromJSON(json) {
        const counter = new GCounter(json.nodeId);
        counter.counters = new Map(Object.entries(json.counters));
        return counter;
    }
}

/**
 * PN-Counter (Positive-Negative Counter)
 * Can increment and decrement
 */
export class PNCounter {
    constructor(nodeId) {
        this.nodeId = nodeId;
        this.positive = new GCounter(nodeId);
        this.negative = new GCounter(nodeId);
    }

    increment(amount = 1) {
        this.positive.increment(amount);
    }

    decrement(amount = 1) {
        this.negative.increment(amount);
    }

    value() {
        return this.positive.value() - this.negative.value();
    }

    merge(other) {
        this.positive.merge(other.positive);
        this.negative.merge(other.negative);
    }

    toJSON() {
        return {
            nodeId: this.nodeId,
            positive: this.positive.toJSON(),
            negative: this.negative.toJSON(),
        };
    }

    static fromJSON(json) {
        const counter = new PNCounter(json.nodeId);
        counter.positive = GCounter.fromJSON(json.positive);
        counter.negative = GCounter.fromJSON(json.negative);
        return counter;
    }
}

/**
 * LWW-Register (Last-Writer-Wins Register)
 * Stores a single value with timestamp
 */
export class LWWRegister {
    constructor(nodeId, value = null) {
        this.nodeId = nodeId;
        this.value = value;
        this.timestamp = Date.now();
    }

    set(value) {
        this.value = value;
        this.timestamp = Date.now();
    }

    get() {
        return this.value;
    }

    merge(other) {
        if (other.timestamp > this.timestamp) {
            this.value = other.value;
            this.timestamp = other.timestamp;
        }
    }

    toJSON() {
        return {
            nodeId: this.nodeId,
            value: this.value,
            timestamp: this.timestamp,
        };
    }

    static fromJSON(json) {
        const register = new LWWRegister(json.nodeId);
        register.value = json.value;
        register.timestamp = json.timestamp;
        return register;
    }
}

/**
 * LWW-Map (Last-Writer-Wins Map)
 * Map with LWW semantics per key
 */
export class LWWMap {
    constructor(nodeId) {
        this.nodeId = nodeId;
        this.entries = new Map(); // key -> { value, timestamp }
    }

    set(key, value) {
        this.entries.set(key, {
            value,
            timestamp: Date.now(),
        });
    }

    get(key) {
        const entry = this.entries.get(key);
        return entry ? entry.value : undefined;
    }

    delete(key) {
        this.entries.set(key, {
            value: null,
            timestamp: Date.now(),
            deleted: true,
        });
    }

    has(key) {
        const entry = this.entries.get(key);
        return entry && !entry.deleted;
    }

    keys() {
        return Array.from(this.entries.keys()).filter(k => !this.entries.get(k).deleted);
    }

    values() {
        return this.keys().map(k => this.entries.get(k).value);
    }

    merge(other) {
        for (const [key, entry] of other.entries) {
            const current = this.entries.get(key);
            if (!current || entry.timestamp > current.timestamp) {
                this.entries.set(key, { ...entry });
            }
        }
    }

    toJSON() {
        return {
            nodeId: this.nodeId,
            entries: Object.fromEntries(this.entries),
        };
    }

    static fromJSON(json) {
        const map = new LWWMap(json.nodeId);
        map.entries = new Map(Object.entries(json.entries));
        return map;
    }
}

// ============================================
// PERSISTENT LEDGER
// ============================================

/**
 * Distributed Ledger with CRDT and persistence
 */
export class Ledger extends EventEmitter {
    constructor(options = {}) {
        super();
        this.nodeId = options.nodeId || `node-${randomBytes(8).toString('hex')}`;

        // Storage path
        this.dataDir = options.dataDir ||
            join(homedir(), '.ruvector', 'edge-net', 'ledger');

        // CRDT state
        this.earned = new GCounter(this.nodeId);
        this.spent = new GCounter(this.nodeId);
        this.metadata = new LWWMap(this.nodeId);
        this.transactions = [];

        // Configuration
        this.autosaveInterval = options.autosaveInterval || 30000; // 30 seconds
        this.maxTransactions = options.maxTransactions || 10000;

        // Sync
        this.lastSync = 0;
        this.syncPeers = new Set();

        // Initialize
        this.initialized = false;
    }

    /**
     * Initialize ledger and load from disk
     */
    async initialize() {
        // Create data directory
        if (!existsSync(this.dataDir)) {
            mkdirSync(this.dataDir, { recursive: true });
        }

        // Load existing state
        await this.load();

        // Start autosave
        this.autosaveTimer = setInterval(() => {
            this.save().catch(err => console.error('[Ledger] Autosave error:', err));
        }, this.autosaveInterval);

        this.initialized = true;
        this.emit('ready', { nodeId: this.nodeId });

        return this;
    }

    /**
     * Credit (earn) amount
     */
    credit(amount, memo = '') {
        if (amount <= 0) throw new Error('Amount must be positive');

        this.earned.increment(amount);

        const tx = {
            id: `tx-${randomBytes(8).toString('hex')}`,
            type: 'credit',
            amount,
            memo,
            timestamp: Date.now(),
            nodeId: this.nodeId,
        };

        this.transactions.push(tx);
        this.pruneTransactions();

        this.emit('credit', { amount, balance: this.balance(), tx });

        return tx;
    }

    /**
     * Debit (spend) amount
     */
    debit(amount, memo = '') {
        if (amount <= 0) throw new Error('Amount must be positive');
        if (amount > this.balance()) throw new Error('Insufficient balance');

        this.spent.increment(amount);

        const tx = {
            id: `tx-${randomBytes(8).toString('hex')}`,
            type: 'debit',
            amount,
            memo,
            timestamp: Date.now(),
            nodeId: this.nodeId,
        };

        this.transactions.push(tx);
        this.pruneTransactions();

        this.emit('debit', { amount, balance: this.balance(), tx });

        return tx;
    }

    /**
     * Get current balance
     */
    balance() {
        return this.earned.value() - this.spent.value();
    }

    /**
     * Get total earned
     */
    totalEarned() {
        return this.earned.value();
    }

    /**
     * Get total spent
     */
    totalSpent() {
        return this.spent.value();
    }

    /**
     * Set metadata
     */
    setMetadata(key, value) {
        this.metadata.set(key, value);
        this.emit('metadata', { key, value });
    }

    /**
     * Get metadata
     */
    getMetadata(key) {
        return this.metadata.get(key);
    }

    /**
     * Get recent transactions
     */
    getTransactions(limit = 50) {
        return this.transactions.slice(-limit);
    }

    /**
     * Prune old transactions
     */
    pruneTransactions() {
        if (this.transactions.length > this.maxTransactions) {
            this.transactions = this.transactions.slice(-this.maxTransactions);
        }
    }

    /**
     * Merge with another ledger state (CRDT merge)
     */
    merge(other) {
        // Merge counters
        if (other.earned) {
            this.earned.merge(
                other.earned instanceof GCounter
                    ? other.earned
                    : GCounter.fromJSON(other.earned)
            );
        }

        if (other.spent) {
            this.spent.merge(
                other.spent instanceof GCounter
                    ? other.spent
                    : GCounter.fromJSON(other.spent)
            );
        }

        if (other.metadata) {
            this.metadata.merge(
                other.metadata instanceof LWWMap
                    ? other.metadata
                    : LWWMap.fromJSON(other.metadata)
            );
        }

        // Merge transactions (deduplicate by id)
        if (other.transactions) {
            const existingIds = new Set(this.transactions.map(t => t.id));
            for (const tx of other.transactions) {
                if (!existingIds.has(tx.id)) {
                    this.transactions.push(tx);
                }
            }
            // Sort by timestamp and prune
            this.transactions.sort((a, b) => a.timestamp - b.timestamp);
            this.pruneTransactions();
        }

        this.lastSync = Date.now();
        this.emit('merged', { balance: this.balance() });
    }

    /**
     * Export state for synchronization
     */
    export() {
        return {
            nodeId: this.nodeId,
            timestamp: Date.now(),
            earned: this.earned.toJSON(),
            spent: this.spent.toJSON(),
            metadata: this.metadata.toJSON(),
            transactions: this.transactions,
        };
    }

    /**
     * Save to disk
     */
    async save() {
        const filePath = join(this.dataDir, 'ledger.json');
        const data = this.export();

        writeFileSync(filePath, JSON.stringify(data, null, 2));
        this.emit('saved', { path: filePath });
    }

    /**
     * Load from disk
     */
    async load() {
        const filePath = join(this.dataDir, 'ledger.json');

        if (!existsSync(filePath)) {
            return;
        }

        try {
            const data = JSON.parse(readFileSync(filePath, 'utf-8'));

            this.earned = GCounter.fromJSON(data.earned);
            this.spent = GCounter.fromJSON(data.spent);
            this.metadata = LWWMap.fromJSON(data.metadata);
            this.transactions = data.transactions || [];

            this.emit('loaded', { balance: this.balance() });
        } catch (error) {
            console.error('[Ledger] Load error:', error.message);
        }
    }

    /**
     * Get ledger summary
     */
    getSummary() {
        return {
            nodeId: this.nodeId,
            balance: this.balance(),
            earned: this.totalEarned(),
            spent: this.totalSpent(),
            transactions: this.transactions.length,
            lastSync: this.lastSync,
            initialized: this.initialized,
        };
    }

    /**
     * Shutdown ledger
     */
    async shutdown() {
        if (this.autosaveTimer) {
            clearInterval(this.autosaveTimer);
        }

        await this.save();
        this.initialized = false;
        this.emit('shutdown');
    }
}

// ============================================
// SYNC CLIENT
// ============================================

/**
 * Ledger sync client for relay communication
 */
export class LedgerSyncClient extends EventEmitter {
    constructor(options = {}) {
        super();
        this.ledger = options.ledger;
        this.relayUrl = options.relayUrl || 'ws://localhost:8080';
        this.ws = null;
        this.connected = false;
        this.syncInterval = options.syncInterval || 60000; // 1 minute
    }

    /**
     * Connect to relay for syncing
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

                this.ws = new WebSocket(this.relayUrl);

                const timeout = setTimeout(() => {
                    reject(new Error('Connection timeout'));
                }, 10000);

                this.ws.onopen = () => {
                    clearTimeout(timeout);
                    this.connected = true;

                    // Register for ledger sync
                    this.send({
                        type: 'register',
                        nodeId: this.ledger.nodeId,
                        capabilities: ['ledger_sync'],
                    });

                    this.emit('connected');
                    resolve(true);
                };

                this.ws.onmessage = (event) => {
                    this.handleMessage(JSON.parse(event.data));
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
                this.startSyncLoop();
                break;

            case 'ledger_state':
                this.handleLedgerState(message);
                break;

            case 'ledger_update':
                this.ledger.merge(message.state);
                break;

            default:
                this.emit('message', message);
        }
    }

    /**
     * Handle ledger state from relay
     */
    handleLedgerState(message) {
        if (message.state) {
            this.ledger.merge(message.state);
        }
        this.emit('synced', { balance: this.ledger.balance() });
    }

    /**
     * Start periodic sync
     */
    startSyncLoop() {
        // Initial sync
        this.sync();

        // Periodic sync
        this.syncTimer = setInterval(() => {
            this.sync();
        }, this.syncInterval);
    }

    /**
     * Sync with relay
     */
    sync() {
        if (!this.connected) return;

        this.send({
            type: 'ledger_sync',
            state: this.ledger.export(),
        });
    }

    /**
     * Send message
     */
    send(message) {
        if (this.connected && this.ws?.readyState === 1) {
            this.ws.send(JSON.stringify(message));
            return true;
        }
        return false;
    }

    /**
     * Close connection
     */
    close() {
        if (this.syncTimer) {
            clearInterval(this.syncTimer);
        }
        if (this.ws) {
            this.ws.close();
        }
    }
}

// ============================================
// EXPORTS
// ============================================

export default Ledger;
