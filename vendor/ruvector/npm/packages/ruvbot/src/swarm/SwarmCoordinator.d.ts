/**
 * SwarmCoordinator - Multi-Agent Swarm Orchestration
 *
 * Provides distributed task coordination using agentic-flow patterns.
 * Supports multiple topologies and consensus protocols.
 */
import { EventEmitter } from 'events';
export type SwarmTopology = 'hierarchical' | 'mesh' | 'hierarchical-mesh' | 'adaptive';
export type ConsensusProtocol = 'byzantine' | 'raft' | 'gossip' | 'crdt';
export type WorkerType = 'ultralearn' | 'optimize' | 'consolidate' | 'predict' | 'audit' | 'map' | 'preload' | 'deepdive' | 'document' | 'refactor' | 'benchmark' | 'testgaps';
export type WorkerPriority = 'low' | 'normal' | 'high' | 'critical';
export interface SwarmConfig {
    topology: SwarmTopology;
    maxAgents: number;
    strategy: 'specialized' | 'balanced' | 'adaptive';
    consensus: ConsensusProtocol;
    heartbeatInterval?: number;
    taskTimeout?: number;
}
export interface WorkerConfig {
    type: WorkerType;
    priority: WorkerPriority;
    concurrency: number;
    timeout: number;
    retries: number;
    backoff: 'exponential' | 'linear';
}
export interface SwarmTask {
    id: string;
    worker: WorkerType;
    type: string;
    content: unknown;
    priority: WorkerPriority;
    status: 'pending' | 'running' | 'completed' | 'failed';
    assignedAgent?: string;
    result?: unknown;
    error?: string;
    createdAt: Date;
    startedAt?: Date;
    completedAt?: Date;
}
export interface SwarmAgent {
    id: string;
    type: WorkerType;
    status: 'idle' | 'busy' | 'offline';
    currentTask?: string;
    completedTasks: number;
    failedTasks: number;
    lastHeartbeat: Date;
}
export interface DispatchOptions {
    worker: WorkerType;
    task: {
        type: string;
        content: unknown;
    };
    priority?: WorkerPriority;
    timeout?: number;
}
export declare const WORKER_DEFAULTS: Record<WorkerType, WorkerConfig>;
export declare class SwarmCoordinator extends EventEmitter {
    private readonly config;
    private agents;
    private tasks;
    private taskQueue;
    private heartbeatTimer?;
    private started;
    constructor(config?: Partial<SwarmConfig>);
    /**
     * Start the swarm coordinator
     */
    start(): Promise<void>;
    /**
     * Stop the swarm coordinator
     */
    stop(): Promise<void>;
    /**
     * Spawn a new worker agent
     */
    spawnAgent(type: WorkerType): Promise<SwarmAgent>;
    /**
     * Remove an agent
     */
    removeAgent(agentId: string): Promise<boolean>;
    /**
     * Dispatch a task to the swarm
     */
    dispatch(options: DispatchOptions): Promise<SwarmTask>;
    /**
     * Wait for a task to complete
     */
    waitForTask(taskId: string, timeout?: number): Promise<SwarmTask>;
    /**
     * Complete a task (called by worker)
     */
    completeTask(taskId: string, result?: unknown, error?: string): void;
    /**
     * Get swarm status
     */
    getStatus(): {
        topology: SwarmTopology;
        consensus: ConsensusProtocol;
        agentCount: number;
        maxAgents: number;
        idleAgents: number;
        busyAgents: number;
        pendingTasks: number;
        runningTasks: number;
        completedTasks: number;
        failedTasks: number;
    };
    /**
     * Get all agents
     */
    getAgents(): SwarmAgent[];
    /**
     * Get agent by ID
     */
    getAgent(agentId: string): SwarmAgent | undefined;
    /**
     * Get all tasks
     */
    getTasks(): SwarmTask[];
    /**
     * Get task by ID
     */
    getTask(taskId: string): SwarmTask | undefined;
    /**
     * Update agent heartbeat
     */
    heartbeat(agentId: string): void;
    private enqueueTask;
    private dequeueTask;
    private processQueue;
    private findTaskForAgent;
    private assignTask;
    private checkHeartbeats;
}
export declare function createSwarmCoordinator(config?: Partial<SwarmConfig>): SwarmCoordinator;
export default SwarmCoordinator;
//# sourceMappingURL=SwarmCoordinator.d.ts.map