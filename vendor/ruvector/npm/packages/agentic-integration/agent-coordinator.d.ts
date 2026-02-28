/**
 * Agent Coordinator - Main coordination logic for distributed ruvector agents
 *
 * Handles:
 * - Agent initialization and registration
 * - Task distribution across regions
 * - Load balancing logic
 * - Health monitoring
 * - Failover coordination
 */
import { EventEmitter } from 'events';
export interface AgentMetrics {
    agentId: string;
    region: string;
    cpuUsage: number;
    memoryUsage: number;
    activeStreams: number;
    queryLatency: number;
    timestamp: number;
    healthy: boolean;
}
export interface Task {
    id: string;
    type: 'query' | 'index' | 'sync' | 'maintenance';
    payload: any;
    priority: number;
    region?: string;
    retries: number;
    maxRetries: number;
    createdAt: number;
}
export interface AgentRegistration {
    agentId: string;
    region: string;
    endpoint: string;
    capabilities: string[];
    capacity: number;
    registeredAt: number;
}
export interface CoordinatorConfig {
    maxAgentsPerRegion: number;
    healthCheckInterval: number;
    taskTimeout: number;
    retryBackoffBase: number;
    retryBackoffMax: number;
    loadBalancingStrategy: 'round-robin' | 'least-connections' | 'weighted' | 'adaptive';
    failoverThreshold: number;
    enableClaudeFlowHooks: boolean;
}
export declare class AgentCoordinator extends EventEmitter {
    private config;
    private agents;
    private agentMetrics;
    private taskQueue;
    private activeTasks;
    private healthCheckTimer?;
    private taskDistributionTimer?;
    private regionLoadIndex;
    private circuitBreakers;
    constructor(config: CoordinatorConfig);
    /**
     * Initialize coordinator with claude-flow hooks
     */
    private initializeCoordinator;
    /**
     * Register a new agent in the coordination system
     */
    registerAgent(registration: AgentRegistration): Promise<void>;
    /**
     * Unregister an agent from the coordination system
     */
    unregisterAgent(agentId: string): Promise<void>;
    /**
     * Submit a task for distributed execution
     */
    submitTask(task: Omit<Task, 'id' | 'retries' | 'createdAt'>): Promise<string>;
    /**
     * Insert task into queue maintaining priority order
     */
    private insertTaskByPriority;
    /**
     * Distribute tasks to agents using configured load balancing strategy
     */
    private distributeNextTask;
    /**
     * Select best agent for task based on load balancing strategy
     */
    private selectAgent;
    /**
     * Round-robin load balancing
     */
    private selectAgentRoundRobin;
    /**
     * Least connections load balancing
     */
    private selectAgentLeastConnections;
    /**
     * Weighted load balancing based on agent capacity
     */
    private selectAgentWeighted;
    /**
     * Adaptive load balancing based on real-time metrics
     */
    private selectAgentAdaptive;
    /**
     * Calculate adaptive score for agent selection
     */
    private calculateAdaptiveScore;
    /**
     * Execute task with exponential backoff retry logic
     */
    private executeTaskWithRetry;
    /**
     * Execute task on specific agent (placeholder for actual implementation)
     */
    private executeTaskOnAgent;
    /**
     * Handle task failure
     */
    private handleTaskFailure;
    /**
     * Redistribute task to another agent (failover)
     */
    private redistributeTask;
    /**
     * Failover task when agent is unavailable
     */
    private failoverTask;
    /**
     * Update agent metrics
     */
    updateAgentMetrics(metrics: AgentMetrics): void;
    /**
     * Start health monitoring loop
     */
    private startHealthMonitoring;
    /**
     * Perform health checks on all agents
     */
    private performHealthChecks;
    /**
     * Start task distribution loop
     */
    private startTaskDistribution;
    /**
     * Get coordinator status
     */
    getStatus(): {
        totalAgents: number;
        healthyAgents: number;
        queuedTasks: number;
        activeTasks: number;
        regionDistribution: Record<string, number>;
    };
    /**
     * Shutdown coordinator gracefully
     */
    shutdown(): Promise<void>;
}
//# sourceMappingURL=agent-coordinator.d.ts.map