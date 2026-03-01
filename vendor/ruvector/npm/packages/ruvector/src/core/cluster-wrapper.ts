/**
 * Cluster Wrapper - Distributed coordination for multi-agent systems
 *
 * Wraps @ruvector/cluster for Raft consensus, auto-sharding,
 * and distributed memory across agents.
 */

let clusterModule: any = null;
let loadError: Error | null = null;

function getClusterModule() {
  if (clusterModule) return clusterModule;
  if (loadError) throw loadError;

  try {
    clusterModule = require('@ruvector/cluster');
    return clusterModule;
  } catch (e: any) {
    loadError = new Error(
      `@ruvector/cluster not installed: ${e.message}\n` +
      `Install with: npm install @ruvector/cluster`
    );
    throw loadError;
  }
}

export function isClusterAvailable(): boolean {
  try {
    getClusterModule();
    return true;
  } catch {
    return false;
  }
}

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
export class RuvectorCluster {
  private inner: any;
  private nodeId: string;
  private isLeader: boolean = false;

  constructor(config: ClusterConfig) {
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
  async start(): Promise<void> {
    await this.inner.start();
  }

  /**
   * Stop the cluster node gracefully
   */
  async stop(): Promise<void> {
    await this.inner.stop();
  }

  /**
   * Join an existing cluster
   */
  async join(peerAddress: string): Promise<boolean> {
    return this.inner.join(peerAddress);
  }

  /**
   * Leave the cluster
   */
  async leave(): Promise<void> {
    await this.inner.leave();
  }

  // ===========================================================================
  // Node Management
  // ===========================================================================

  /**
   * Get current node info
   */
  getNodeInfo(): ClusterNode {
    return this.inner.getNodeInfo();
  }

  /**
   * Get all cluster nodes
   */
  getNodes(): ClusterNode[] {
    return this.inner.getNodes();
  }

  /**
   * Check if this node is the leader
   */
  isClusterLeader(): boolean {
    this.isLeader = this.inner.isLeader();
    return this.isLeader;
  }

  /**
   * Get the current leader
   */
  getLeader(): ClusterNode | null {
    return this.inner.getLeader();
  }

  // ===========================================================================
  // Distributed Operations
  // ===========================================================================

  /**
   * Put a value in distributed storage
   */
  async put(key: string, value: any): Promise<boolean> {
    return this.inner.put(key, JSON.stringify(value));
  }

  /**
   * Get a value from distributed storage
   */
  async get(key: string): Promise<any | null> {
    const result = await this.inner.get(key);
    return result ? JSON.parse(result) : null;
  }

  /**
   * Delete a value from distributed storage
   */
  async delete(key: string): Promise<boolean> {
    return this.inner.delete(key);
  }

  /**
   * Atomic compare-and-swap
   */
  async compareAndSwap(key: string, expected: any, newValue: any): Promise<boolean> {
    return this.inner.compareAndSwap(
      key,
      JSON.stringify(expected),
      JSON.stringify(newValue)
    );
  }

  // ===========================================================================
  // Sharding
  // ===========================================================================

  /**
   * Get shard information
   */
  getShards(): ShardInfo[] {
    return this.inner.getShards();
  }

  /**
   * Get the shard for a key
   */
  getShardForKey(key: string): ShardInfo {
    return this.inner.getShardForKey(key);
  }

  /**
   * Trigger shard rebalancing
   */
  async rebalance(): Promise<void> {
    await this.inner.rebalance();
  }

  // ===========================================================================
  // Distributed Locks
  // ===========================================================================

  /**
   * Acquire a distributed lock
   */
  async lock(name: string, timeout: number = 30000): Promise<string | null> {
    return this.inner.lock(name, timeout);
  }

  /**
   * Release a distributed lock
   */
  async unlock(name: string, token: string): Promise<boolean> {
    return this.inner.unlock(name, token);
  }

  /**
   * Extend a lock's TTL
   */
  async extendLock(name: string, token: string, extension: number = 30000): Promise<boolean> {
    return this.inner.extendLock(name, token, extension);
  }

  // ===========================================================================
  // Pub/Sub
  // ===========================================================================

  /**
   * Subscribe to a channel
   */
  subscribe(channel: string, callback: (message: any) => void): () => void {
    return this.inner.subscribe(channel, (msg: string) => {
      callback(JSON.parse(msg));
    });
  }

  /**
   * Publish to a channel
   */
  async publish(channel: string, message: any): Promise<number> {
    return this.inner.publish(channel, JSON.stringify(message));
  }

  // ===========================================================================
  // Agent Coordination
  // ===========================================================================

  /**
   * Register an agent with the cluster
   */
  async registerAgent(agentId: string, capabilities: string[]): Promise<boolean> {
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
  async findAgents(capability: string): Promise<string[]> {
    const agents = await this.inner.scan('agent:*');
    const matching: string[] = [];

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
  async assignTask(taskId: string, agentId: string, task: any): Promise<boolean> {
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
  async completeTask(taskId: string, result: any): Promise<boolean> {
    const task = await this.get(`task:${taskId}`);
    if (!task) return false;

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
  stats(): {
    nodes: number;
    shards: number;
    leader: string | null;
    healthy: boolean;
  } {
    return this.inner.stats();
  }
}

/**
 * Create a cluster node for agent coordination
 */
export function createCluster(config: ClusterConfig): RuvectorCluster {
  return new RuvectorCluster(config);
}

export default RuvectorCluster;
