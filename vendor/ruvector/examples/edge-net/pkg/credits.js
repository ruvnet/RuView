/**
 * @ruvector/edge-net Credit System MVP
 *
 * Simple credit accounting for distributed task execution:
 * - Nodes earn credits when executing tasks for others
 * - Nodes spend credits when submitting tasks
 * - Credits stored in CRDT ledger for conflict-free replication
 * - Persisted to Firebase for cross-session continuity
 *
 * @module @ruvector/edge-net/credits
 */

import { EventEmitter } from 'events';
import { Ledger } from './ledger.js';

// ============================================
// CREDIT CONFIGURATION
// ============================================

/**
 * Default credit values for operations
 */
export const CREDIT_CONFIG = {
    // Base credit cost per task submission
    taskSubmissionCost: 1,

    // Credits earned per task completion (base rate)
    taskCompletionReward: 1,

    // Multipliers for task types
    taskTypeMultipliers: {
        embed: 1.0,
        process: 1.0,
        analyze: 1.5,
        transform: 1.0,
        compute: 2.0,
        aggregate: 1.5,
        custom: 1.0,
    },

    // Priority multipliers (higher priority = higher cost/reward)
    priorityMultipliers: {
        low: 0.5,
        medium: 1.0,
        high: 1.5,
        critical: 2.0,
    },

    // Initial credits for new nodes (bootstrap)
    initialCredits: 10,

    // Minimum balance required to submit tasks (0 = no minimum)
    minimumBalance: 0,

    // Maximum transaction history to keep per node
    maxTransactionHistory: 1000,
};

// ============================================
// CREDIT SYSTEM
// ============================================

/**
 * CreditSystem - Manages credit accounting for distributed task execution
 *
 * Integrates with:
 * - Ledger (CRDT) for conflict-free credit tracking
 * - TaskExecutionHandler for automatic credit operations
 * - FirebaseLedgerSync for persistence
 */
export class CreditSystem extends EventEmitter {
    /**
     * @param {Object} options
     * @param {string} options.nodeId - This node's identifier
     * @param {Ledger} options.ledger - CRDT ledger instance (will create if not provided)
     * @param {Object} options.config - Credit configuration overrides
     */
    constructor(options = {}) {
        super();

        this.nodeId = options.nodeId;
        this.config = { ...CREDIT_CONFIG, ...options.config };

        // Use provided ledger or create new one
        this.ledger = options.ledger || new Ledger({
            nodeId: this.nodeId,
            maxTransactions: this.config.maxTransactionHistory,
        });

        // Transaction tracking by taskId (for deduplication)
        this.processedTasks = new Map(); // taskId -> { type, timestamp }

        // Stats
        this.stats = {
            creditsEarned: 0,
            creditsSpent: 0,
            tasksExecuted: 0,
            tasksSubmitted: 0,
            insufficientFunds: 0,
        };

        this.initialized = false;
    }

    /**
     * Initialize credit system
     */
    async initialize() {
        // Initialize ledger
        if (!this.ledger.initialized) {
            await this.ledger.initialize();
        }

        // Grant initial credits if balance is zero (new node)
        if (this.ledger.balance() === 0 && this.config.initialCredits > 0) {
            this.ledger.credit(this.config.initialCredits, 'Initial bootstrap credits');
            console.log(`[Credits] Granted ${this.config.initialCredits} initial credits`);
        }

        this.initialized = true;
        this.emit('initialized', { balance: this.getBalance() });

        return this;
    }

    // ============================================
    // CREDIT OPERATIONS
    // ============================================

    /**
     * Earn credits when completing a task for another node
     *
     * @param {string} nodeId - The node that earned credits (usually this node)
     * @param {number} amount - Credit amount (will be adjusted by multipliers)
     * @param {string} taskId - Task identifier
     * @param {Object} taskInfo - Task details for calculating multipliers
     * @returns {Object} Transaction record
     */
    earnCredits(nodeId, amount, taskId, taskInfo = {}) {
        // Only process for this node
        if (nodeId !== this.nodeId) {
            console.warn(`[Credits] Ignoring earnCredits for different node: ${nodeId}`);
            return null;
        }

        // Check for duplicate processing
        if (this.processedTasks.has(`earn:${taskId}`)) {
            console.warn(`[Credits] Task ${taskId} already credited`);
            return null;
        }

        // Calculate final amount with multipliers
        const finalAmount = this._calculateAmount(amount, taskInfo);

        // Record transaction in ledger
        const tx = this.ledger.credit(finalAmount, JSON.stringify({
            taskId,
            type: 'task_completion',
            taskType: taskInfo.type,
            submitter: taskInfo.submitter,
        }));

        // Mark as processed
        this.processedTasks.set(`earn:${taskId}`, {
            type: 'earn',
            amount: finalAmount,
            timestamp: Date.now(),
        });

        // Update stats
        this.stats.creditsEarned += finalAmount;
        this.stats.tasksExecuted++;

        // Prune old processed tasks (keep last 10000)
        this._pruneProcessedTasks();

        this.emit('credits-earned', {
            nodeId,
            amount: finalAmount,
            taskId,
            balance: this.getBalance(),
            tx,
        });

        console.log(`[Credits] Earned ${finalAmount} credits for task ${taskId.slice(0, 8)}...`);

        return tx;
    }

    /**
     * Spend credits when submitting a task
     *
     * @param {string} nodeId - The node spending credits (usually this node)
     * @param {number} amount - Credit amount (will be adjusted by multipliers)
     * @param {string} taskId - Task identifier
     * @param {Object} taskInfo - Task details for calculating cost
     * @returns {Object|null} Transaction record or null if insufficient funds
     */
    spendCredits(nodeId, amount, taskId, taskInfo = {}) {
        // Only process for this node
        if (nodeId !== this.nodeId) {
            console.warn(`[Credits] Ignoring spendCredits for different node: ${nodeId}`);
            return null;
        }

        // Check for duplicate processing
        if (this.processedTasks.has(`spend:${taskId}`)) {
            console.warn(`[Credits] Task ${taskId} already charged`);
            return null;
        }

        // Calculate final amount with multipliers
        const finalAmount = this._calculateAmount(amount, taskInfo);

        // Check balance
        const balance = this.getBalance();
        if (balance < finalAmount) {
            this.stats.insufficientFunds++;
            this.emit('insufficient-funds', {
                nodeId,
                required: finalAmount,
                available: balance,
                taskId,
            });

            // In MVP, we allow tasks even with insufficient funds
            // (can be enforced later)
            if (this.config.minimumBalance > 0 && balance < this.config.minimumBalance) {
                console.warn(`[Credits] Insufficient funds: ${balance} < ${finalAmount}`);
                return null;
            }
        }

        // Record transaction in ledger
        let tx;
        try {
            tx = this.ledger.debit(finalAmount, JSON.stringify({
                taskId,
                type: 'task_submission',
                taskType: taskInfo.type,
                targetPeer: taskInfo.targetPeer,
            }));
        } catch (error) {
            // Debit failed (insufficient balance in strict mode)
            console.warn(`[Credits] Debit failed: ${error.message}`);
            return null;
        }

        // Mark as processed
        this.processedTasks.set(`spend:${taskId}`, {
            type: 'spend',
            amount: finalAmount,
            timestamp: Date.now(),
        });

        // Update stats
        this.stats.creditsSpent += finalAmount;
        this.stats.tasksSubmitted++;

        this._pruneProcessedTasks();

        this.emit('credits-spent', {
            nodeId,
            amount: finalAmount,
            taskId,
            balance: this.getBalance(),
            tx,
        });

        console.log(`[Credits] Spent ${finalAmount} credits for task ${taskId.slice(0, 8)}...`);

        return tx;
    }

    /**
     * Get current credit balance
     *
     * @param {string} nodeId - Node to check (defaults to this node)
     * @returns {number} Current balance
     */
    getBalance(nodeId = null) {
        // For MVP, only track this node's balance
        if (nodeId && nodeId !== this.nodeId) {
            // Would need network query for other nodes
            return 0;
        }
        return this.ledger.balance();
    }

    /**
     * Get transaction history
     *
     * @param {string} nodeId - Node to get history for (defaults to this node)
     * @param {number} limit - Maximum transactions to return
     * @returns {Array} Transaction history
     */
    getTransactionHistory(nodeId = null, limit = 50) {
        // For MVP, only track this node's history
        if (nodeId && nodeId !== this.nodeId) {
            return [];
        }

        const transactions = this.ledger.getTransactions(limit);

        // Parse memo JSON and add readable info
        return transactions.map(tx => {
            let details = {};
            try {
                details = JSON.parse(tx.memo || '{}');
            } catch {
                details = { memo: tx.memo };
            }

            return {
                id: tx.id,
                type: tx.type, // 'credit' or 'debit'
                amount: tx.amount,
                timestamp: tx.timestamp,
                date: new Date(tx.timestamp).toISOString(),
                ...details,
            };
        });
    }

    /**
     * Check if node has sufficient credits for a task
     *
     * @param {number} amount - Base amount
     * @param {Object} taskInfo - Task info for multipliers
     * @returns {boolean} True if sufficient
     */
    hasSufficientCredits(amount, taskInfo = {}) {
        const required = this._calculateAmount(amount, taskInfo);
        return this.getBalance() >= required;
    }

    // ============================================
    // CALCULATION HELPERS
    // ============================================

    /**
     * Calculate final credit amount with multipliers
     */
    _calculateAmount(baseAmount, taskInfo = {}) {
        let amount = baseAmount;

        // Apply task type multiplier
        if (taskInfo.type && this.config.taskTypeMultipliers[taskInfo.type]) {
            amount *= this.config.taskTypeMultipliers[taskInfo.type];
        }

        // Apply priority multiplier
        if (taskInfo.priority && this.config.priorityMultipliers[taskInfo.priority]) {
            amount *= this.config.priorityMultipliers[taskInfo.priority];
        }

        // Round to 2 decimal places
        return Math.round(amount * 100) / 100;
    }

    /**
     * Prune old processed task records
     */
    _pruneProcessedTasks() {
        if (this.processedTasks.size > 10000) {
            // Remove oldest entries
            const entries = Array.from(this.processedTasks.entries())
                .sort((a, b) => a[1].timestamp - b[1].timestamp);

            const toRemove = entries.slice(0, 5000);
            for (const [key] of toRemove) {
                this.processedTasks.delete(key);
            }
        }
    }

    // ============================================
    // INTEGRATION METHODS
    // ============================================

    /**
     * Wire to TaskExecutionHandler for automatic credit operations
     *
     * @param {TaskExecutionHandler} handler - Task execution handler
     */
    wireToTaskHandler(handler) {
        // Auto-credit when we complete a task
        handler.on('task-complete', ({ taskId, from, duration, result }) => {
            this.earnCredits(
                this.nodeId,
                this.config.taskCompletionReward,
                taskId,
                {
                    type: result?.taskType || 'compute',
                    submitter: from,
                    duration,
                }
            );
        });

        // Could also track task submissions if handler emits that event
        handler.on('task-submitted', ({ taskId, to, task }) => {
            this.spendCredits(
                this.nodeId,
                this.config.taskSubmissionCost,
                taskId,
                {
                    type: task?.type || 'compute',
                    priority: task?.priority,
                    targetPeer: to,
                }
            );
        });

        console.log('[Credits] Wired to TaskExecutionHandler');
    }

    /**
     * Get credit system summary
     */
    getSummary() {
        return {
            nodeId: this.nodeId,
            balance: this.getBalance(),
            totalEarned: this.ledger.totalEarned(),
            totalSpent: this.ledger.totalSpent(),
            stats: { ...this.stats },
            initialized: this.initialized,
            recentTransactions: this.getTransactionHistory(null, 5),
        };
    }

    /**
     * Export ledger state for sync
     */
    export() {
        return this.ledger.export();
    }

    /**
     * Merge with remote ledger state (CRDT)
     */
    merge(remoteState) {
        this.ledger.merge(remoteState);
        this.emit('merged', { balance: this.getBalance() });
    }

    /**
     * Shutdown credit system
     */
    async shutdown() {
        await this.ledger.shutdown();
        this.initialized = false;
        this.emit('shutdown');
    }
}

// ============================================
// FIREBASE CREDIT SYNC
// ============================================

/**
 * Syncs credits to Firebase for persistence and cross-node visibility
 */
export class FirebaseCreditSync extends EventEmitter {
    /**
     * @param {CreditSystem} creditSystem - Credit system to sync
     * @param {Object} options
     * @param {Object} options.firebaseConfig - Firebase configuration
     * @param {number} options.syncInterval - Sync interval in ms
     */
    constructor(creditSystem, options = {}) {
        super();

        this.credits = creditSystem;
        this.config = options.firebaseConfig;
        this.syncInterval = options.syncInterval || 30000;

        // Firebase instances
        this.db = null;
        this.firebase = null;
        this.syncTimer = null;
        this.unsubscribers = [];
    }

    /**
     * Start Firebase sync
     */
    async start() {
        if (!this.config || !this.config.apiKey || !this.config.projectId) {
            console.log('[FirebaseCreditSync] No Firebase config, skipping sync');
            return false;
        }

        try {
            const { initializeApp, getApps } = await import('firebase/app');
            const { getFirestore, doc, setDoc, onSnapshot, getDoc, collection } = await import('firebase/firestore');

            this.firebase = { doc, setDoc, onSnapshot, getDoc, collection };

            const apps = getApps();
            const app = apps.length ? apps[0] : initializeApp(this.config);
            this.db = getFirestore(app);

            // Initial sync
            await this.pull();

            // Subscribe to updates
            this.subscribe();

            // Periodic push
            this.syncTimer = setInterval(() => this.push(), this.syncInterval);

            console.log('[FirebaseCreditSync] Started');
            return true;

        } catch (error) {
            console.log('[FirebaseCreditSync] Failed to start:', error.message);
            return false;
        }
    }

    /**
     * Pull credit state from Firebase
     */
    async pull() {
        const { doc, getDoc } = this.firebase;

        const creditRef = doc(this.db, 'edgenet_credits', this.credits.nodeId);
        const snapshot = await getDoc(creditRef);

        if (snapshot.exists()) {
            const remoteState = snapshot.data();
            if (remoteState.ledgerState) {
                this.credits.merge(remoteState.ledgerState);
            }
        }
    }

    /**
     * Push credit state to Firebase
     */
    async push() {
        const { doc, setDoc } = this.firebase;

        const creditRef = doc(this.db, 'edgenet_credits', this.credits.nodeId);

        await setDoc(creditRef, {
            nodeId: this.credits.nodeId,
            balance: this.credits.getBalance(),
            totalEarned: this.credits.ledger.totalEarned(),
            totalSpent: this.credits.ledger.totalSpent(),
            ledgerState: this.credits.export(),
            updatedAt: Date.now(),
        }, { merge: true });
    }

    /**
     * Subscribe to credit updates from Firebase
     */
    subscribe() {
        const { doc, onSnapshot } = this.firebase;

        const creditRef = doc(this.db, 'edgenet_credits', this.credits.nodeId);

        const unsubscribe = onSnapshot(creditRef, (snapshot) => {
            if (snapshot.exists()) {
                const data = snapshot.data();
                if (data.ledgerState) {
                    this.credits.merge(data.ledgerState);
                }
            }
        });

        this.unsubscribers.push(unsubscribe);
    }

    /**
     * Stop sync
     */
    stop() {
        if (this.syncTimer) {
            clearInterval(this.syncTimer);
            this.syncTimer = null;
        }

        for (const unsub of this.unsubscribers) {
            if (typeof unsub === 'function') unsub();
        }
        this.unsubscribers = [];
    }
}

// ============================================
// CONVENIENCE FACTORY
// ============================================

/**
 * Create and initialize a complete credit system with optional Firebase sync
 *
 * @param {Object} options
 * @param {string} options.nodeId - Node identifier
 * @param {Ledger} options.ledger - Existing ledger (optional)
 * @param {Object} options.firebaseConfig - Firebase config for sync
 * @param {Object} options.config - Credit configuration overrides
 * @returns {Promise<CreditSystem>} Initialized credit system
 */
export async function createCreditSystem(options = {}) {
    const system = new CreditSystem(options);
    await system.initialize();

    // Start Firebase sync if configured
    if (options.firebaseConfig) {
        const sync = new FirebaseCreditSync(system, {
            firebaseConfig: options.firebaseConfig,
            syncInterval: options.syncInterval,
        });
        await sync.start();

        // Attach sync to system for cleanup
        system._firebaseSync = sync;
    }

    return system;
}

// ============================================
// EXPORTS
// ============================================

export default CreditSystem;
