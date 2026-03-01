/**
 * @ruvector/edge-net QDAG (Quantum DAG) Implementation
 *
 * Directed Acyclic Graph for distributed consensus and task tracking
 * Inspired by IOTA Tangle and DAG-based blockchains
 *
 * Features:
 * - Tip selection algorithm
 * - Proof of contribution verification
 * - Transaction validation
 * - Network synchronization
 *
 * @module @ruvector/edge-net/qdag
 */

import { EventEmitter } from 'events';
import { randomBytes, createHash, createHmac } from 'crypto';

// ============================================
// TRANSACTION
// ============================================

/**
 * QDAG Transaction
 */
export class Transaction {
    constructor(data = {}) {
        this.id = data.id || `tx-${randomBytes(16).toString('hex')}`;
        this.timestamp = data.timestamp || Date.now();
        this.type = data.type || 'generic'; // 'genesis', 'task', 'reward', 'transfer'

        // Links to parent transactions (must reference 2 tips)
        this.parents = data.parents || [];

        // Transaction payload
        this.payload = data.payload || {};

        // Proof of contribution
        this.proof = data.proof || null;

        // Issuer
        this.issuer = data.issuer || null;
        this.signature = data.signature || null;

        // Computed fields
        this.hash = data.hash || this.computeHash();
        this.weight = data.weight || 1;
        this.cumulativeWeight = data.cumulativeWeight || 1;
        this.confirmed = data.confirmed || false;
        this.confirmedAt = data.confirmedAt || null;
    }

    /**
     * Compute transaction hash
     */
    computeHash() {
        const content = JSON.stringify({
            id: this.id,
            timestamp: this.timestamp,
            type: this.type,
            parents: this.parents,
            payload: this.payload,
            proof: this.proof,
            issuer: this.issuer,
        });

        return createHash('sha256').update(content).digest('hex');
    }

    /**
     * Sign transaction
     */
    sign(privateKey) {
        const hmac = createHmac('sha256', privateKey);
        hmac.update(this.hash);
        this.signature = hmac.digest('hex');
        return this.signature;
    }

    /**
     * Verify signature
     */
    verify(publicKey) {
        if (!this.signature) return false;

        const hmac = createHmac('sha256', publicKey);
        hmac.update(this.hash);
        const expected = hmac.digest('hex');

        return this.signature === expected;
    }

    /**
     * Serialize transaction
     */
    toJSON() {
        return {
            id: this.id,
            timestamp: this.timestamp,
            type: this.type,
            parents: this.parents,
            payload: this.payload,
            proof: this.proof,
            issuer: this.issuer,
            signature: this.signature,
            hash: this.hash,
            weight: this.weight,
            cumulativeWeight: this.cumulativeWeight,
            confirmed: this.confirmed,
            confirmedAt: this.confirmedAt,
        };
    }

    /**
     * Deserialize transaction
     */
    static fromJSON(json) {
        return new Transaction(json);
    }
}

// ============================================
// QDAG (Quantum DAG)
// ============================================

/**
 * QDAG - Directed Acyclic Graph for distributed consensus
 */
export class QDAG extends EventEmitter {
    constructor(options = {}) {
        super();
        this.id = options.id || `qdag-${randomBytes(8).toString('hex')}`;
        this.nodeId = options.nodeId;

        // Transaction storage
        this.transactions = new Map();
        this.tips = new Set();           // Unconfirmed transactions
        this.confirmed = new Set();       // Confirmed transactions

        // Indices
        this.byIssuer = new Map();        // issuer -> Set<txId>
        this.byType = new Map();          // type -> Set<txId>
        this.children = new Map();        // txId -> Set<childTxId>

        // Configuration
        this.confirmationThreshold = options.confirmationThreshold || 5;
        this.maxTips = options.maxTips || 100;
        this.pruneAge = options.pruneAge || 24 * 60 * 60 * 1000; // 24 hours

        // Stats
        this.stats = {
            transactions: 0,
            confirmed: 0,
            tips: 0,
            avgConfirmationTime: 0,
        };

        // Create genesis if needed
        if (options.createGenesis !== false) {
            this.createGenesis();
        }
    }

    /**
     * Create genesis transaction
     */
    createGenesis() {
        const genesis = new Transaction({
            id: 'genesis',
            type: 'genesis',
            parents: [],
            payload: {
                message: 'QDAG Genesis',
                timestamp: Date.now(),
            },
            issuer: 'system',
        });

        genesis.confirmed = true;
        genesis.confirmedAt = Date.now();
        genesis.cumulativeWeight = this.confirmationThreshold + 1;

        this.transactions.set(genesis.id, genesis);
        this.tips.add(genesis.id);
        this.confirmed.add(genesis.id);

        this.emit('genesis', genesis);

        return genesis;
    }

    /**
     * Select tips for new transaction (weighted random walk)
     */
    selectTips(count = 2) {
        // Ensure genesis exists
        if (!this.transactions.has('genesis')) {
            this.createGenesis();
        }

        const tips = Array.from(this.tips);

        // Fallback to genesis if no tips available
        if (tips.length === 0) {
            return ['genesis'];
        }

        // Return all tips if we have fewer than requested
        if (tips.length <= count) {
            return [...tips]; // Return copy to prevent mutation issues
        }

        // Weighted random selection based on cumulative weight
        const selected = new Set();
        const weights = tips.map(tipId => {
            const tx = this.transactions.get(tipId);
            return tx ? Math.max(tx.cumulativeWeight, 1) : 1;
        });

        const totalWeight = weights.reduce((a, b) => a + b, 0);

        // Safety: prevent infinite loop
        let attempts = 0;
        const maxAttempts = count * 10;

        while (selected.size < count && selected.size < tips.length && attempts < maxAttempts) {
            let random = Math.random() * totalWeight;

            for (let i = 0; i < tips.length; i++) {
                random -= weights[i];
                if (random <= 0) {
                    selected.add(tips[i]);
                    break;
                }
            }
            attempts++;
        }

        // Ensure we have at least one valid parent
        const result = Array.from(selected);
        if (result.length === 0) {
            result.push(tips[0] || 'genesis');
        }

        return result;
    }

    /**
     * Add transaction to QDAG
     */
    addTransaction(tx) {
        // Validate transaction with detailed error
        const validation = this.validateTransaction(tx, { returnError: true });
        if (!validation.valid) {
            throw new Error(`Invalid transaction: ${validation.error}`);
        }

        // Check for duplicates
        if (this.transactions.has(tx.id)) {
            return this.transactions.get(tx.id);
        }

        // Store transaction
        this.transactions.set(tx.id, tx);
        this.tips.add(tx.id);
        this.stats.transactions++;

        // Update indices
        if (tx.issuer) {
            if (!this.byIssuer.has(tx.issuer)) {
                this.byIssuer.set(tx.issuer, new Set());
            }
            this.byIssuer.get(tx.issuer).add(tx.id);
        }

        if (!this.byType.has(tx.type)) {
            this.byType.set(tx.type, new Set());
        }
        this.byType.get(tx.type).add(tx.id);

        // Update parent references
        for (const parentId of tx.parents) {
            if (!this.children.has(parentId)) {
                this.children.set(parentId, new Set());
            }
            this.children.get(parentId).add(tx.id);

            // Remove parent from tips
            this.tips.delete(parentId);
        }

        // Update weights
        this.updateWeights(tx.id);

        // Check for confirmations
        this.checkConfirmations();

        this.emit('transaction', tx);

        return tx;
    }

    /**
     * Create and add a new transaction
     */
    createTransaction(type, payload, options = {}) {
        const parents = options.parents || this.selectTips(2);

        const tx = new Transaction({
            type,
            payload,
            parents,
            issuer: options.issuer || this.nodeId,
            proof: options.proof,
        });

        if (options.privateKey) {
            tx.sign(options.privateKey);
        }

        return this.addTransaction(tx);
    }

    /**
     * Validate transaction
     * @returns {boolean|{valid: boolean, error: string}}
     */
    validateTransaction(tx, options = {}) {
        const returnError = options.returnError || false;
        const fail = (msg) => returnError ? { valid: false, error: msg } : false;
        const pass = () => returnError ? { valid: true, error: null } : true;

        // Check required fields
        if (!tx.id) {
            return fail('Missing transaction id');
        }
        if (!tx.timestamp) {
            return fail('Missing transaction timestamp');
        }
        if (!tx.type) {
            return fail('Missing transaction type');
        }

        // Genesis transactions don't need parents
        if (tx.type === 'genesis') {
            return pass();
        }

        // Ensure genesis exists before validating non-genesis transactions
        if (!this.transactions.has('genesis')) {
            this.createGenesis();
        }

        // Check parents exist
        if (!tx.parents || tx.parents.length === 0) {
            return fail('Non-genesis transaction must have at least one parent');
        }

        for (const parentId of tx.parents) {
            if (!this.transactions.has(parentId)) {
                return fail(`Parent transaction not found: ${parentId}`);
            }
        }

        // Check no cycles (parents must be older or equal for simultaneous txs)
        for (const parentId of tx.parents) {
            const parent = this.transactions.get(parentId);
            // Allow equal timestamps (transactions created at same time)
            if (parent && parent.timestamp > tx.timestamp) {
                return fail(`Parent ${parentId} has future timestamp`);
            }
        }

        return pass();
    }

    /**
     * Update cumulative weights
     */
    updateWeights(txId) {
        const tx = this.transactions.get(txId);
        if (!tx) return;

        // Update weight of this transaction
        tx.cumulativeWeight = tx.weight;

        // Add weight of all children
        const children = this.children.get(txId);
        if (children) {
            for (const childId of children) {
                const child = this.transactions.get(childId);
                if (child) {
                    tx.cumulativeWeight += child.cumulativeWeight;
                }
            }
        }

        // Propagate to parents
        for (const parentId of tx.parents) {
            this.updateWeights(parentId);
        }
    }

    /**
     * Check for newly confirmed transactions
     */
    checkConfirmations() {
        for (const [txId, tx] of this.transactions) {
            if (!tx.confirmed && tx.cumulativeWeight >= this.confirmationThreshold) {
                tx.confirmed = true;
                tx.confirmedAt = Date.now();

                this.confirmed.add(txId);
                this.stats.confirmed++;

                // Update average confirmation time
                const confirmTime = tx.confirmedAt - tx.timestamp;
                this.stats.avgConfirmationTime =
                    (this.stats.avgConfirmationTime * (this.stats.confirmed - 1) + confirmTime) /
                    this.stats.confirmed;

                this.emit('confirmed', tx);
            }
        }

        this.stats.tips = this.tips.size;
    }

    /**
     * Get transaction by ID
     */
    getTransaction(txId) {
        return this.transactions.get(txId);
    }

    /**
     * Get transactions by issuer
     */
    getByIssuer(issuer) {
        const txIds = this.byIssuer.get(issuer) || new Set();
        return Array.from(txIds).map(id => this.transactions.get(id));
    }

    /**
     * Get transactions by type
     */
    getByType(type) {
        const txIds = this.byType.get(type) || new Set();
        return Array.from(txIds).map(id => this.transactions.get(id));
    }

    /**
     * Get current tips
     */
    getTips() {
        return Array.from(this.tips).map(id => this.transactions.get(id));
    }

    /**
     * Get confirmed transactions
     */
    getConfirmed() {
        return Array.from(this.confirmed).map(id => this.transactions.get(id));
    }

    /**
     * Prune old transactions
     */
    prune() {
        const cutoff = Date.now() - this.pruneAge;
        let pruned = 0;

        for (const [txId, tx] of this.transactions) {
            if (tx.type === 'genesis') continue;

            if (tx.confirmed && tx.confirmedAt < cutoff) {
                // Remove from storage
                this.transactions.delete(txId);
                this.confirmed.delete(txId);
                this.tips.delete(txId);

                // Clean up indices
                if (tx.issuer && this.byIssuer.has(tx.issuer)) {
                    this.byIssuer.get(tx.issuer).delete(txId);
                }
                if (this.byType.has(tx.type)) {
                    this.byType.get(tx.type).delete(txId);
                }

                this.children.delete(txId);

                pruned++;
            }
        }

        if (pruned > 0) {
            this.emit('pruned', { count: pruned });
        }

        return pruned;
    }

    /**
     * Get QDAG statistics
     */
    getStats() {
        return {
            id: this.id,
            ...this.stats,
            size: this.transactions.size,
            memoryUsage: process.memoryUsage?.().heapUsed,
        };
    }

    /**
     * Export QDAG for synchronization
     */
    export(since = 0) {
        const transactions = [];

        for (const [txId, tx] of this.transactions) {
            if (tx.timestamp >= since) {
                transactions.push(tx.toJSON());
            }
        }

        return {
            id: this.id,
            timestamp: Date.now(),
            transactions,
        };
    }

    /**
     * Import transactions from another node
     */
    import(data) {
        let imported = 0;

        // Sort by timestamp to maintain order
        const sorted = data.transactions.sort((a, b) => a.timestamp - b.timestamp);

        for (const txData of sorted) {
            try {
                const tx = Transaction.fromJSON(txData);
                if (!this.transactions.has(tx.id)) {
                    this.addTransaction(tx);
                    imported++;
                }
            } catch (error) {
                console.error('[QDAG] Import error:', error.message);
            }
        }

        this.emit('imported', { count: imported, from: data.id });

        return imported;
    }

    /**
     * Merge with another QDAG
     */
    merge(other) {
        return this.import(other.export());
    }
}

// ============================================
// TASK TRANSACTION HELPERS
// ============================================

/**
 * Create a task submission transaction
 */
export function createTaskTransaction(qdag, task, options = {}) {
    return qdag.createTransaction('task', {
        taskId: task.id,
        type: task.type,
        data: task.data,
        priority: task.priority || 'medium',
        reward: task.reward || 0,
        deadline: task.deadline,
    }, options);
}

/**
 * Create a task completion/reward transaction
 */
export function createRewardTransaction(qdag, taskTxId, result, options = {}) {
    const taskTx = qdag.getTransaction(taskTxId);
    if (!taskTx) throw new Error('Task transaction not found');

    return qdag.createTransaction('reward', {
        taskTxId,
        result,
        worker: options.worker,
        reward: taskTx.payload.reward || 0,
        completedAt: Date.now(),
    }, {
        ...options,
        parents: [taskTxId, ...qdag.selectTips(1)],
    });
}

/**
 * Create a credit transfer transaction
 */
export function createTransferTransaction(qdag, from, to, amount, options = {}) {
    return qdag.createTransaction('transfer', {
        from,
        to,
        amount,
        memo: options.memo,
    }, options);
}

// ============================================
// EXPORTS
// ============================================

export default QDAG;
