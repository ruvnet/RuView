"use strict";
/**
 * Cluster Wrapper - Distributed coordination for multi-agent systems
 *
 * Wraps @ruvector/cluster for Raft consensus, auto-sharding,
 * and distributed memory across agents.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.RuvectorCluster = void 0;
exports.isClusterAvailable = isClusterAvailable;
exports.createCluster = createCluster;
let clusterModule = null;
let loadError = null;
function getClusterModule() {
    if (clusterModule)
        return clusterModule;
    if (loadError)
        throw loadError;
    try {
        clusterModule = require('@ruvector/cluster');
        return clusterModule;
    }
    catch (e) {
        loadError = new Error(`@ruvector/cluster not installed: ${e.message}\n` +
            `Install with: npm install @ruvector/cluster`);
        throw loadError;
    }
}
function isClusterAvailable() {
    try {
        getClusterModule();
        return true;
    }
    catch {
        return false;
    }
}
/**
 * Distributed cluster for multi-agent coordination
 */
class RuvectorCluster {
    constructor(config) {
        this.isLeader = false;
        const cluster = getClusterModule();
        this.nodeId = config.nodeId;
        this.inner = new cluster.Cluster({
            nodeId: config.nodeId,
            address: config.address,
            peers: config.peers ?? [],
            shards: config.shards ?? 16,
            replicationFactor: config.replicationFactor ?? 2,
        });
    }
    // ===========================================================================
    // Cluster Lifecycle
    // ===========================================================================
    /**
     * Start the cluster node
     */
    async start() {
        await this.inner.start();
    }
    /**
     * Stop the cluster node gracefully
     */
    async stop() {
        await this.inner.stop();
    }
    /**
     * Join an existing cluster
     */
    async join(peerAddress) {
        return this.inner.join(peerAddress);
    }
    /**
     * Leave the cluster
     */
    async leave() {
        await this.inner.leave();
    }
    // ===========================================================================
    // Node Management
    // ===========================================================================
    /**
     * Get current node info
     */
    getNodeInfo() {
        return this.inner.getNodeInfo();
    }
    /**
     * Get all cluster nodes
     */
    getNodes() {
        return this.inner.getNodes();
    }
    /**
     * Check if this node is the leader
     */
    isClusterLeader() {
        this.isLeader = this.inner.isLeader();
        return this.isLeader;
    }
    /**
     * Get the current leader
     */
    getLeader() {
        return this.inner.getLeader();
    }
    // ===========================================================================
    // Distributed Operations
    // ===========================================================================
    /**
     * Put a value in distributed storage
     */
    async put(key, value) {
        return this.inner.put(key, JSON.stringify(value));
    }
    /**
     * Get a value from distributed storage
     */
    async get(key) {
        const result = await this.inner.get(key);
        return result ? JSON.parse(result) : null;
    }
    /**
     * Delete a value from distributed storage
     */
    async delete(key) {
        return this.inner.delete(key);
    }
    /**
     * Atomic compare-and-swap
     */
    async compareAndSwap(key, expected, newValue) {
        return this.inner.compareAndSwap(key, JSON.stringify(expected), JSON.stringify(newValue));
    }
    // ===========================================================================
    // Sharding
    // ===========================================================================
    /**
     * Get shard information
     */
    getShards() {
        return this.inner.getShards();
    }
    /**
     * Get the shard for a key
     */
    getShardForKey(key) {
        return this.inner.getShardForKey(key);
    }
    /**
     * Trigger shard rebalancing
     */
    async rebalance() {
        await this.inner.rebalance();
    }
    // ===========================================================================
    // Distributed Locks
    // ===========================================================================
    /**
     * Acquire a distributed lock
     */
    async lock(name, timeout = 30000) {
        return this.inner.lock(name, timeout);
    }
    /**
     * Release a distributed lock
     */
    async unlock(name, token) {
        return this.inner.unlock(name, token);
    }
    /**
     * Extend a lock's TTL
     */
    async extendLock(name, token, extension = 30000) {
        return this.inner.extendLock(name, token, extension);
    }
    // ===========================================================================
    // Pub/Sub
    // ===========================================================================
    /**
     * Subscribe to a channel
     */
    subscribe(channel, callback) {
        return this.inner.subscribe(channel, (msg) => {
            callback(JSON.parse(msg));
        });
    }
    /**
     * Publish to a channel
     */
    async publish(channel, message) {
        return this.inner.publish(channel, JSON.stringify(message));
    }
    // ===========================================================================
    // Agent Coordination
    // ===========================================================================
    /**
     * Register an agent with the cluster
     */
    async registerAgent(agentId, capabilities) {
        return this.put(`agent:${agentId}`, {
            id: agentId,
            capabilities,
            node: this.nodeId,
            registeredAt: Date.now(),
        });
    }
    /**
     * Find agents with a capability
     */
    async findAgents(capability) {
        const agents = await this.inner.scan('agent:*');
        const matching = [];
        for (const key of agents) {
            const agent = await this.get(key);
            if (agent?.capabilities?.includes(capability)) {
                matching.push(agent.id);
            }
        }
        return matching;
    }
    /**
     * Assign a task to an agent
     */
    async assignTask(taskId, agentId, task) {
        const assigned = await this.put(`task:${taskId}`, {
            id: taskId,
            agent: agentId,
            task,
            status: 'assigned',
            assignedAt: Date.now(),
        });
        if (assigned) {
            await this.publish(`agent:${agentId}:tasks`, { type: 'new_task', taskId });
        }
        return assigned;
    }
    /**
     * Complete a task
     */
    async completeTask(taskId, result) {
        const task = await this.get(`task:${taskId}`);
        if (!task)
            return false;
        return this.put(`task:${taskId}`, {
            ...task,
            status: 'completed',
            result,
            completedAt: Date.now(),
        });
    }
    // ===========================================================================
    // Stats
    // ===========================================================================
    /**
     * Get cluster statistics
     */
    stats() {
        return this.inner.stats();
    }
}
exports.RuvectorCluster = RuvectorCluster;
/**
 * Create a cluster node for agent coordination
 */
function createCluster(config) {
    return new RuvectorCluster(config);
}
exports.default = RuvectorCluster;
//# sourceMappingURL=cluster-wrapper.js.map