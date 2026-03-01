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
import { exec } from 'child_process';
import { promisify } from 'util';
import { RegionalAgent, RegionalAgentConfig } from './regional-agent';
import { AgentCoordinator, AgentRegistration } from './agent-coordinator';

const execAsync = promisify(exec);

export interface SwarmConfig {
  topology: 'mesh' | 'hierarchical' | 'hybrid';
  minAgentsPerRegion: number;
  maxAgentsPerRegion: number;
  scaleUpThreshold: number; // CPU/memory threshold to trigger scale-up
  scaleDownThreshold: number; // Threshold to trigger scale-down
  scaleUpCooldown: number; // Cooldown period between scale-ups (ms)
  scaleDownCooldown: number; // Cooldown period between scale-downs (ms)
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

export class SwarmManager extends EventEmitter {
  private agents: Map<string, RegionalAgent> = new Map();
  private agentConfigs: Map<string, RegionalAgentConfig> = new Map();
  private lastScaleUp: Map<string, number> = new Map();
  private lastScaleDown: Map<string, number> = new Map();
  private healthCheckTimer?: NodeJS.Timeout;
  private autoScaleTimer?: NodeJS.Timeout;
  private swarmMemory: Map<string, any> = new Map();
  private agentCounter = 0;

  constructor(
    private config: SwarmConfig,
    private coordinator: AgentCoordinator
  ) {
    super();
    this.initialize();
  }

  /**
   * Initialize swarm manager
   */
  private async initialize(): Promise<void> {
    console.log('[SwarmManager] Initializing swarm manager...');
    console.log(`[SwarmManager] Topology: ${this.config.topology}`);
    console.log(`[SwarmManager] Regions: ${this.config.regions.join(', ')}`);

    if (this.config.enableClaudeFlowHooks) {
      try {
        // Initialize swarm coordination via claude-flow
        await execAsync(
          `npx claude-flow@alpha hooks pre-task --description "Initialize swarm manager with ${this.config.topology} topology"`
        );

        // Initialize swarm topology
        const topologyCmd = JSON.stringify({
          topology: this.config.topology,
          maxAgents: this.config.maxAgentsPerRegion * this.config.regions.length,
        }).replace(/"/g, '\\"');

        console.log('[SwarmManager] Initializing claude-flow swarm coordination...');

        // Store swarm configuration in memory
        await this.storeInMemory('swarm/config', this.config);

        console.log('[SwarmManager] Claude-flow hooks initialized');
      } catch (error) {
        console.warn('[SwarmManager] Claude-flow hooks not available:', error);
      }
    }

    // Spawn initial agents for each region
    await this.spawnInitialAgents();

    // Start health monitoring
    if (this.config.healthCheckInterval > 0) {
      this.startHealthMonitoring();
    }

    // Start auto-scaling
    if (this.config.enableAutoScaling) {
      this.startAutoScaling();
    }

    this.emit('swarm:initialized', {
      topology: this.config.topology,
      regions: this.config.regions,
      initialAgents: this.agents.size,
    });

    console.log(`[SwarmManager] Swarm initialized with ${this.agents.size} agents`);
  }

  /**
   * Spawn initial agents for each region
   */
  private async spawnInitialAgents(): Promise<void> {
    console.log('[SwarmManager] Spawning initial agents...');

    const spawnPromises: Promise<void>[] = [];

    for (const region of this.config.regions) {
      for (let i = 0; i < this.config.minAgentsPerRegion; i++) {
        spawnPromises.push(this.spawnAgent(region));
      }
    }

    await Promise.all(spawnPromises);

    console.log(`[SwarmManager] Spawned ${this.agents.size} initial agents`);
  }

  /**
   * Spawn a new agent in specific region
   */
  async spawnAgent(region: string, capacity: number = 1000): Promise<string> {
    const agentId = `agent-${region}-${this.agentCounter++}`;

    console.log(`[SwarmManager] Spawning agent ${agentId} in ${region}`);

    const agentConfig: RegionalAgentConfig = {
      agentId,
      region,
      coordinatorEndpoint: 'coordinator.ruvector.io',
      localStoragePath: `/var/lib/ruvector/${region}/${agentId}`,
      maxConcurrentStreams: 1000,
      metricsReportInterval: 30000, // 30 seconds
      syncInterval: 5000, // 5 seconds
      enableClaudeFlowHooks: this.config.enableClaudeFlowHooks,
      vectorDimensions: 768, // Default dimension
      capabilities: ['query', 'index', 'sync'],
    };

    // Create agent instance
    const agent = new RegionalAgent(agentConfig);

    // Set up event handlers
    this.setupAgentEventHandlers(agent, agentConfig);

    // Store agent
    this.agents.set(agentId, agent);
    this.agentConfigs.set(agentId, agentConfig);

    // Register with coordinator
    const registration: AgentRegistration = {
      agentId,
      region,
      endpoint: `https://${region}.ruvector.io/agent/${agentId}`,
      capabilities: agentConfig.capabilities,
      capacity,
      registeredAt: Date.now(),
    };

    await this.coordinator.registerAgent(registration);

    if (this.config.enableClaudeFlowHooks) {
      try {
        // Notify about agent spawn
        await execAsync(
          `npx claude-flow@alpha hooks notify --message "Spawned agent ${agentId} in ${region}"`
        );

        // Store agent info in swarm memory
        await this.storeInMemory(`swarm/agents/${agentId}`, {
          config: agentConfig,
          registration,
          spawnedAt: Date.now(),
        });
      } catch (error) {
        // Non-critical
      }
    }

    this.emit('agent:spawned', { agentId, region });

    return agentId;
  }

  /**
   * Set up event handlers for agent
   */
  private setupAgentEventHandlers(agent: RegionalAgent, config: RegionalAgentConfig): void {
    // Forward agent events to swarm manager
    agent.on('metrics:report', (metrics) => {
      this.coordinator.updateAgentMetrics(metrics);
    });

    agent.on('query:completed', (data) => {
      this.emit('query:completed', { ...data, agentId: config.agentId });
    });

    agent.on('query:failed', (data) => {
      this.emit('query:failed', { ...data, agentId: config.agentId });
    });

    agent.on('sync:broadcast', (payload) => {
      this.handleSyncBroadcast(payload, config.region);
    });

    agent.on('agent:shutdown', () => {
      this.handleAgentShutdown(config.agentId);
    });
  }

  /**
   * Handle sync broadcast from agent
   */
  private async handleSyncBroadcast(payload: any, sourceRegion: string): Promise<void> {
    // Broadcast to all agents in other regions
    for (const [agentId, agent] of this.agents.entries()) {
      const agentConfig = this.agentConfigs.get(agentId);

      if (agentConfig && agentConfig.region !== sourceRegion) {
        try {
          await agent.handleSyncPayload(payload);
        } catch (error) {
          console.error(`[SwarmManager] Error syncing to agent ${agentId}:`, error);
        }
      }
    }
  }

  /**
   * Despawn an agent
   */
  async despawnAgent(agentId: string): Promise<void> {
    console.log(`[SwarmManager] Despawning agent ${agentId}`);

    const agent = this.agents.get(agentId);
    if (!agent) {
      throw new Error(`Agent ${agentId} not found`);
    }

    // Unregister from coordinator
    await this.coordinator.unregisterAgent(agentId);

    // Shutdown agent
    await agent.shutdown();

    // Remove from tracking
    this.agents.delete(agentId);
    this.agentConfigs.delete(agentId);

    if (this.config.enableClaudeFlowHooks) {
      try {
        await execAsync(
          `npx claude-flow@alpha hooks notify --message "Despawned agent ${agentId}"`
        );

        // Remove from swarm memory
        await this.removeFromMemory(`swarm/agents/${agentId}`);
      } catch (error) {
        // Non-critical
      }
    }

    this.emit('agent:despawned', { agentId });
  }

  /**
   * Handle agent shutdown
   */
  private handleAgentShutdown(agentId: string): void {
    console.log(`[SwarmManager] Agent ${agentId} has shut down`);

    this.agents.delete(agentId);
    this.agentConfigs.delete(agentId);

    this.emit('agent:shutdown', { agentId });
  }

  /**
   * Start health monitoring
   */
  private startHealthMonitoring(): void {
    this.healthCheckTimer = setInterval(() => {
      this.performHealthChecks();
    }, this.config.healthCheckInterval);
  }

  /**
   * Perform health checks on all agents
   */
  private async performHealthChecks(): Promise<void> {
    const unhealthyAgents: string[] = [];

    for (const [agentId, agent] of this.agents.entries()) {
      const status = agent.getStatus();

      if (!status.healthy) {
        unhealthyAgents.push(agentId);
        console.warn(`[SwarmManager] Agent ${agentId} is unhealthy`);
      }
    }

    if (unhealthyAgents.length > 0) {
      this.emit('health:check', {
        unhealthyAgents,
        totalAgents: this.agents.size,
      });
    }

    // Could implement auto-recovery here
    // for (const agentId of unhealthyAgents) {
    //   await this.recoverAgent(agentId);
    // }
  }

  /**
   * Start auto-scaling
   */
  private startAutoScaling(): void {
    this.autoScaleTimer = setInterval(() => {
      this.evaluateScaling();
    }, 10000); // Evaluate every 10 seconds
  }

  /**
   * Evaluate if scaling is needed
   */
  private async evaluateScaling(): Promise<void> {
    const metrics = this.calculateSwarmMetrics();

    for (const [region, regionMetrics] of Object.entries(metrics.regionMetrics)) {
      const avgLoad = (regionMetrics.avgCpuUsage + regionMetrics.avgMemoryUsage) / 2;

      // Check scale-up condition
      if (
        avgLoad > this.config.scaleUpThreshold &&
        regionMetrics.agentCount < this.config.maxAgentsPerRegion &&
        this.canScaleUp(region)
      ) {
        console.log(`[SwarmManager] Scaling up in region ${region} (load: ${avgLoad.toFixed(1)}%)`);
        await this.scaleUp(region);
      }

      // Check scale-down condition
      if (
        avgLoad < this.config.scaleDownThreshold &&
        regionMetrics.agentCount > this.config.minAgentsPerRegion &&
        this.canScaleDown(region)
      ) {
        console.log(`[SwarmManager] Scaling down in region ${region} (load: ${avgLoad.toFixed(1)}%)`);
        await this.scaleDown(region);
      }
    }
  }

  /**
   * Check if can scale up (respects cooldown)
   */
  private canScaleUp(region: string): boolean {
    const lastScaleUp = this.lastScaleUp.get(region) || 0;
    return Date.now() - lastScaleUp > this.config.scaleUpCooldown;
  }

  /**
   * Check if can scale down (respects cooldown)
   */
  private canScaleDown(region: string): boolean {
    const lastScaleDown = this.lastScaleDown.get(region) || 0;
    return Date.now() - lastScaleDown > this.config.scaleDownCooldown;
  }

  /**
   * Scale up agents in region
   */
  private async scaleUp(region: string): Promise<void> {
    try {
      await this.spawnAgent(region);
      this.lastScaleUp.set(region, Date.now());

      this.emit('swarm:scale-up', { region, totalAgents: this.agents.size });
    } catch (error) {
      console.error(`[SwarmManager] Error scaling up in ${region}:`, error);
    }
  }

  /**
   * Scale down agents in region
   */
  private async scaleDown(region: string): Promise<void> {
    // Find agent with lowest load in region
    const regionAgents = Array.from(this.agents.entries())
      .filter(([_, agent]) => {
        const config = this.agentConfigs.get(agent.getStatus().agentId);
        return config?.region === region;
      })
      .map(([agentId, agent]) => ({
        agentId,
        status: agent.getStatus(),
      }))
      .sort((a, b) => a.status.activeStreams - b.status.activeStreams);

    if (regionAgents.length > 0) {
      const agentToDespawn = regionAgents[0];

      try {
        await this.despawnAgent(agentToDespawn.agentId);
        this.lastScaleDown.set(region, Date.now());

        this.emit('swarm:scale-down', { region, totalAgents: this.agents.size });
      } catch (error) {
        console.error(`[SwarmManager] Error scaling down in ${region}:`, error);
      }
    }
  }

  /**
   * Calculate swarm metrics
   */
  calculateSwarmMetrics(): SwarmMetrics {
    const regionMetrics: Record<string, RegionMetrics> = {};
    let totalLoad = 0;
    let activeAgents = 0;

    // Initialize region metrics
    for (const region of this.config.regions) {
      regionMetrics[region] = {
        region,
        agentCount: 0,
        activeAgents: 0,
        avgCpuUsage: 0,
        avgMemoryUsage: 0,
        totalStreams: 0,
        avgQueryLatency: 0,
      };
    }

    // Aggregate metrics
    for (const [agentId, agent] of this.agents.entries()) {
      const status = agent.getStatus();
      const config = this.agentConfigs.get(agentId);

      if (!config) continue;

      const regionMetric = regionMetrics[config.region];
      regionMetric.agentCount++;

      if (status.healthy) {
        activeAgents++;
        regionMetric.activeAgents++;
      }

      regionMetric.totalStreams += status.activeStreams;
      regionMetric.avgQueryLatency += status.avgQueryLatency;

      // Note: In production, we would get actual CPU/memory metrics
      totalLoad += status.activeStreams;
    }

    // Calculate averages
    for (const region of this.config.regions) {
      const metric = regionMetrics[region];
      if (metric.agentCount > 0) {
        metric.avgQueryLatency /= metric.agentCount;
        // Placeholder for actual CPU/memory aggregation
        metric.avgCpuUsage = Math.random() * 100;
        metric.avgMemoryUsage = Math.random() * 100;
      }
    }

    return {
      totalAgents: this.agents.size,
      activeAgents,
      totalLoad,
      averageLoad: this.agents.size > 0 ? totalLoad / this.agents.size : 0,
      regionMetrics,
      timestamp: Date.now(),
    };
  }

  /**
   * Store data in swarm memory via claude-flow hooks
   */
  private async storeInMemory(key: string, value: any): Promise<void> {
    this.swarmMemory.set(key, value);

    if (this.config.enableClaudeFlowHooks) {
      try {
        const serialized = JSON.stringify(value).replace(/"/g, '\\"');
        await execAsync(
          `npx claude-flow@alpha hooks post-edit --file "swarm-memory" --memory-key "${key}"`
        );
      } catch (error) {
        console.warn(`[SwarmManager] Error storing in memory: ${key}`, error);
      }
    }
  }

  /**
   * Retrieve data from swarm memory
   */
  private async retrieveFromMemory(key: string): Promise<any> {
    return this.swarmMemory.get(key);
  }

  /**
   * Remove data from swarm memory
   */
  private async removeFromMemory(key: string): Promise<void> {
    this.swarmMemory.delete(key);
  }

  /**
   * Get swarm status
   */
  getStatus(): {
    topology: string;
    regions: string[];
    totalAgents: number;
    metrics: SwarmMetrics;
  } {
    return {
      topology: this.config.topology,
      regions: this.config.regions,
      totalAgents: this.agents.size,
      metrics: this.calculateSwarmMetrics(),
    };
  }

  /**
   * Shutdown swarm gracefully
   */
  async shutdown(): Promise<void> {
    console.log('[SwarmManager] Shutting down swarm...');

    // Stop timers
    if (this.healthCheckTimer) {
      clearInterval(this.healthCheckTimer);
    }
    if (this.autoScaleTimer) {
      clearInterval(this.autoScaleTimer);
    }

    // Shutdown all agents
    const shutdownPromises = Array.from(this.agents.keys()).map(agentId =>
      this.despawnAgent(agentId)
    );

    await Promise.all(shutdownPromises);

    if (this.config.enableClaudeFlowHooks) {
      try {
        await execAsync(
          `npx claude-flow@alpha hooks post-task --task-id "swarm-shutdown"`
        );
        await execAsync(
          `npx claude-flow@alpha hooks session-end --export-metrics true`
        );
      } catch (error) {
        console.warn('[SwarmManager] Error executing shutdown hooks:', error);
      }
    }

    this.emit('swarm:shutdown');

    console.log('[SwarmManager] Swarm shutdown complete');
  }
}
