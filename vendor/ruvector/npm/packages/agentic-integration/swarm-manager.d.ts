/**
 * Swarm Manager - Dynamic agent swarm management
 *
 * Handles:
 * - Dynamic agent spawning based on load
 * - Agent lifecycle management
 * - Topology management (mesh coordination)
 * - Memory/state sharing via claude-flow hooks
 */
import { EventEmitter } from 'events';
import { AgentCoordinator } from './agent-coordinator';
export interface SwarmConfig {
    topology: 'mesh' | 'hierarchical' | 'hybrid';
    minAgentsPerRegion: number;
    maxAgentsPerRegion: number;
    scaleUpThreshold: number;
    scaleDownThreshold: number;
    scaleUpCooldown: number;
    scaleDownCooldown: number;
    healthCheckInterval: number;
    enableAutoScaling: boolean;
    enableClaudeFlowHooks: boolean;
    regions: string[];
}
export interface SwarmMetrics {
    totalAgents: number;
    activeAgents: number;
    totalLoad: number;
    averageLoad: number;
    regionMetrics: Record<string, RegionMetrics>;
    timestamp: number;
}
export interface RegionMetrics {
    region: string;
    agentCount: number;
    activeAgents: number;
    avgCpuUsage: number;
    avgMemoryUsage: number;
    totalStreams: number;
    avgQueryLatency: number;
}
export declare class SwarmManager extends EventEmitter {
    private config;
    private coordinator;
    private agents;
    private agentConfigs;
    private lastScaleUp;
    private lastScaleDown;
    private healthCheckTimer?;
    private autoScaleTimer?;
    private swarmMemory;
    private agentCounter;
    constructor(config: SwarmConfig, coordinator: AgentCoordinator);
    /**
     * Initialize swarm manager
     */
    private initialize;
    /**
     * Spawn initial agents for each region
     */
    private spawnInitialAgents;
    /**
     * Spawn a new agent in specific region
     */
    spawnAgent(region: string, capacity?: number): Promise<string>;
    /**
     * Set up event handlers for agent
     */
    private setupAgentEventHandlers;
    /**
     * Handle sync broadcast from agent
     */
    private handleSyncBroadcast;
    /**
     * Despawn an agent
     */
    despawnAgent(agentId: string): Promise<void>;
    /**
     * Handle agent shutdown
     */
    private handleAgentShutdown;
    /**
     * Start health monitoring
     */
    private startHealthMonitoring;
    /**
     * Perform health checks on all agents
     */
    private performHealthChecks;
    /**
     * Start auto-scaling
     */
    private startAutoScaling;
    /**
     * Evaluate if scaling is needed
     */
    private evaluateScaling;
    /**
     * Check if can scale up (respects cooldown)
     */
    private canScaleUp;
    /**
     * Check if can scale down (respects cooldown)
     */
    private canScaleDown;
    /**
     * Scale up agents in region
     */
    private scaleUp;
    /**
     * Scale down agents in region
     */
    private scaleDown;
    /**
     * Calculate swarm metrics
     */
    calculateSwarmMetrics(): SwarmMetrics;
    /**
     * Store data in swarm memory via claude-flow hooks
     */
    private storeInMemory;
    /**
     * Retrieve data from swarm memory
     */
    private retrieveFromMemory;
    /**
     * Remove data from swarm memory
     */
    private removeFromMemory;
    /**
     * Get swarm status
     */
    getStatus(): {
        topology: string;
        regions: string[];
        totalAgents: number;
        metrics: SwarmMetrics;
    };
    /**
     * Shutdown swarm gracefully
     */
    shutdown(): Promise<void>;
}
//# sourceMappingURL=swarm-manager.d.ts.map