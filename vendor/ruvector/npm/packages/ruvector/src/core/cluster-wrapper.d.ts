/**
 * Cluster Wrapper - Distributed coordination for multi-agent systems
 *
 * Wraps @ruvector/cluster for Raft consensus, auto-sharding,
 * and distributed memory across agents.
 */
export declare function isClusterAvailable(): boolean;
export interface ClusterNode {
    id: string;
    address: string;
    role: 'leader' | 'follower' | 'candidate';
    status: 'healthy' | 'unhealthy' | 'unknown';
    lastHeartbeat: number;
}
export interface ShardInfo {
    id: number;
    range: [number, number];
    node: string;
    size: number;
    status: 'active' | 'migrating' | 'offline';
}
export interface ClusterConfig {
    nodeId: string;
    address: string;
    peers?: string[];
    shards?: number;
    replicationFactor?: number;
}
/**
 * Distributed cluster for multi-agent coordination
 */
export declare class RuvectorCluster {
    private inner;
    private nodeId;
    private isLeader;
    constructor(config: ClusterConfig);
    /**
     * Start the cluster node
     */
    start(): Promise<void>;
    /**
     * Stop the cluster node gracefully
     */
    stop(): Promise<void>;
    /**
     * Join an existing cluster
     */
    join(peerAddress: string): Promise<boolean>;
    /**
     * Leave the cluster
     */
    leave(): Promise<void>;
    /**
     * Get current node info
     */
    getNodeInfo(): ClusterNode;
    /**
     * Get all cluster nodes
     */
    getNodes(): ClusterNode[];
    /**
     * Check if this node is the leader
     */
    isClusterLeader(): boolean;
    /**
     * Get the current leader
     */
    getLeader(): ClusterNode | null;
    /**
     * Put a value in distributed storage
     */
    put(key: string, value: any): Promise<boolean>;
    /**
     * Get a value from distributed storage
     */
    get(key: string): Promise<any | null>;
    /**
     * Delete a value from distributed storage
     */
    delete(key: string): Promise<boolean>;
    /**
     * Atomic compare-and-swap
     */
    compareAndSwap(key: string, expected: any, newValue: any): Promise<boolean>;
    /**
     * Get shard information
     */
    getShards(): ShardInfo[];
    /**
     * Get the shard for a key
     */
    getShardForKey(key: string): ShardInfo;
    /**
     * Trigger shard rebalancing
     */
    rebalance(): Promise<void>;
    /**
     * Acquire a distributed lock
     */
    lock(name: string, timeout?: number): Promise<string | null>;
    /**
     * Release a distributed lock
     */
    unlock(name: string, token: string): Promise<boolean>;
    /**
     * Extend a lock's TTL
     */
    extendLock(name: string, token: string, extension?: number): Promise<boolean>;
    /**
     * Subscribe to a channel
     */
    subscribe(channel: string, callback: (message: any) => void): () => void;
    /**
     * Publish to a channel
     */
    publish(channel: string, message: any): Promise<number>;
    /**
     * Register an agent with the cluster
     */
    registerAgent(agentId: string, capabilities: string[]): Promise<boolean>;
    /**
     * Find agents with a capability
     */
    findAgents(capability: string): Promise<string[]>;
    /**
     * Assign a task to an agent
     */
    assignTask(taskId: string, agentId: string, task: any): Promise<boolean>;
    /**
     * Complete a task
     */
    completeTask(taskId: string, result: any): Promise<boolean>;
    /**
     * Get cluster statistics
     */
    stats(): {
        nodes: number;
        shards: number;
        leader: string | null;
        healthy: boolean;
    };
}
/**
 * Create a cluster node for agent coordination
 */
export declare function createCluster(config: ClusterConfig): RuvectorCluster;
export default RuvectorCluster;
//# sourceMappingURL=cluster-wrapper.d.ts.map