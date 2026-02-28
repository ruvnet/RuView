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
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

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

export class AgentCoordinator extends EventEmitter {
  private agents: Map<string, AgentRegistration> = new Map();
  private agentMetrics: Map<string, AgentMetrics> = new Map();
  private taskQueue: Task[] = [];
  private activeTasks: Map<string, Task> = new Map();
  private healthCheckTimer?: NodeJS.Timeout;
  private taskDistributionTimer?: NodeJS.Timeout;
  private regionLoadIndex: Map<string, number> = new Map();
  private circuitBreakers: Map<string, CircuitBreaker> = new Map();

  constructor(private config: CoordinatorConfig) {
    super();
    this.initializeCoordinator();
  }

  /**
   * Initialize coordinator with claude-flow hooks
   */
  private async initializeCoordinator(): Promise<void> {
    console.log('[AgentCoordinator] Initializing coordinator...');

    if (this.config.enableClaudeFlowHooks) {
      try {
        // Pre-task hook for coordination initialization
        await execAsync(
          `npx claude-flow@alpha hooks pre-task --description "Initialize agent coordinator"`
        );
        console.log('[AgentCoordinator] Claude-flow pre-task hook executed');
      } catch (error) {
        console.warn('[AgentCoordinator] Claude-flow hooks not available:', error);
      }
    }

    // Start health monitoring
    this.startHealthMonitoring();

    // Start task distribution
    this.startTaskDistribution();

    this.emit('coordinator:initialized');
  }

  /**
   * Register a new agent in the coordination system
   */
  async registerAgent(registration: AgentRegistration): Promise<void> {
    console.log(`[AgentCoordinator] Registering agent: ${registration.agentId} in ${registration.region}`);

    // Check if region has capacity
    const regionAgents = Array.from(this.agents.values()).filter(
      a => a.region === registration.region
    );

    if (regionAgents.length >= this.config.maxAgentsPerRegion) {
      throw new Error(`Region ${registration.region} has reached max agent capacity`);
    }

    this.agents.set(registration.agentId, registration);

    // Initialize circuit breaker for agent
    this.circuitBreakers.set(
      registration.agentId,
      new CircuitBreaker({
        threshold: this.config.failoverThreshold,
        timeout: this.config.taskTimeout,
      })
    );

    // Initialize metrics
    this.agentMetrics.set(registration.agentId, {
      agentId: registration.agentId,
      region: registration.region,
      cpuUsage: 0,
      memoryUsage: 0,
      activeStreams: 0,
      queryLatency: 0,
      timestamp: Date.now(),
      healthy: true,
    });

    this.emit('agent:registered', registration);

    console.log(`[AgentCoordinator] Agent ${registration.agentId} registered successfully`);
  }

  /**
   * Unregister an agent from the coordination system
   */
  async unregisterAgent(agentId: string): Promise<void> {
    console.log(`[AgentCoordinator] Unregistering agent: ${agentId}`);

    const agent = this.agents.get(agentId);
    if (!agent) {
      throw new Error(`Agent ${agentId} not found`);
    }

    // Redistribute active tasks
    const agentTasks = Array.from(this.activeTasks.values()).filter(
      task => task.region === agent.region
    );

    for (const task of agentTasks) {
      await this.redistributeTask(task);
    }

    this.agents.delete(agentId);
    this.agentMetrics.delete(agentId);
    this.circuitBreakers.delete(agentId);

    this.emit('agent:unregistered', { agentId });
  }

  /**
   * Submit a task for distributed execution
   */
  async submitTask(task: Omit<Task, 'id' | 'retries' | 'createdAt'>): Promise<string> {
    const fullTask: Task = {
      ...task,
      id: `task-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      retries: 0,
      createdAt: Date.now(),
    };

    console.log(`[AgentCoordinator] Submitting task: ${fullTask.id} (type: ${fullTask.type})`);

    // Add to queue based on priority
    this.insertTaskByPriority(fullTask);

    this.emit('task:submitted', fullTask);

    return fullTask.id;
  }

  /**
   * Insert task into queue maintaining priority order
   */
  private insertTaskByPriority(task: Task): void {
    let insertIndex = this.taskQueue.findIndex(t => t.priority < task.priority);
    if (insertIndex === -1) {
      this.taskQueue.push(task);
    } else {
      this.taskQueue.splice(insertIndex, 0, task);
    }
  }

  /**
   * Distribute tasks to agents using configured load balancing strategy
   */
  private async distributeNextTask(): Promise<void> {
    if (this.taskQueue.length === 0) return;

    const task = this.taskQueue.shift()!;

    try {
      // Select agent based on load balancing strategy
      const agent = await this.selectAgent(task);

      if (!agent) {
        console.warn(`[AgentCoordinator] No available agent for task ${task.id}, requeuing`);
        this.insertTaskByPriority(task);
        return;
      }

      // Check circuit breaker
      const circuitBreaker = this.circuitBreakers.get(agent.agentId);
      if (circuitBreaker && !circuitBreaker.canExecute()) {
        console.warn(`[AgentCoordinator] Circuit breaker open for agent ${agent.agentId}`);
        await this.failoverTask(task, agent.agentId);
        return;
      }

      // Assign task to agent
      this.activeTasks.set(task.id, { ...task, region: agent.region });

      this.emit('task:assigned', {
        taskId: task.id,
        agentId: agent.agentId,
        region: agent.region,
      });

      // Execute task with timeout and retry logic
      await this.executeTaskWithRetry(task, agent);

    } catch (error) {
      console.error(`[AgentCoordinator] Error distributing task ${task.id}:`, error);
      await this.handleTaskFailure(task, error);
    }
  }

  /**
   * Select best agent for task based on load balancing strategy
   */
  private async selectAgent(task: Task): Promise<AgentRegistration | null> {
    const availableAgents = Array.from(this.agents.values()).filter(agent => {
      const metrics = this.agentMetrics.get(agent.agentId);
      return metrics?.healthy && (!task.region || agent.region === task.region);
    });

    if (availableAgents.length === 0) return null;

    switch (this.config.loadBalancingStrategy) {
      case 'round-robin':
        return this.selectAgentRoundRobin(availableAgents, task);

      case 'least-connections':
        return this.selectAgentLeastConnections(availableAgents);

      case 'weighted':
        return this.selectAgentWeighted(availableAgents);

      case 'adaptive':
        return this.selectAgentAdaptive(availableAgents);

      default:
        return availableAgents[0];
    }
  }

  /**
   * Round-robin load balancing
   */
  private selectAgentRoundRobin(agents: AgentRegistration[], task: Task): AgentRegistration {
    const region = task.region || 'default';
    const currentIndex = this.regionLoadIndex.get(region) || 0;
    const regionAgents = agents.filter(a => !task.region || a.region === task.region);

    const selectedAgent = regionAgents[currentIndex % regionAgents.length];
    this.regionLoadIndex.set(region, (currentIndex + 1) % regionAgents.length);

    return selectedAgent;
  }

  /**
   * Least connections load balancing
   */
  private selectAgentLeastConnections(agents: AgentRegistration[]): AgentRegistration {
    return agents.reduce((best, agent) => {
      const bestMetrics = this.agentMetrics.get(best.agentId);
      const agentMetrics = this.agentMetrics.get(agent.agentId);

      return (agentMetrics?.activeStreams || 0) < (bestMetrics?.activeStreams || 0)
        ? agent
        : best;
    });
  }

  /**
   * Weighted load balancing based on agent capacity
   */
  private selectAgentWeighted(agents: AgentRegistration[]): AgentRegistration {
    const totalCapacity = agents.reduce((sum, a) => sum + a.capacity, 0);
    let random = Math.random() * totalCapacity;

    for (const agent of agents) {
      random -= agent.capacity;
      if (random <= 0) return agent;
    }

    return agents[agents.length - 1];
  }

  /**
   * Adaptive load balancing based on real-time metrics
   */
  private selectAgentAdaptive(agents: AgentRegistration[]): AgentRegistration {
    return agents.reduce((best, agent) => {
      const bestMetrics = this.agentMetrics.get(best.agentId);
      const agentMetrics = this.agentMetrics.get(agent.agentId);

      if (!bestMetrics || !agentMetrics) return best;

      // Score based on: low CPU, low memory, low streams, low latency
      const bestScore = this.calculateAdaptiveScore(bestMetrics);
      const agentScore = this.calculateAdaptiveScore(agentMetrics);

      return agentScore > bestScore ? agent : best;
    });
  }

  /**
   * Calculate adaptive score for agent selection
   */
  private calculateAdaptiveScore(metrics: AgentMetrics): number {
    return (
      (100 - metrics.cpuUsage) * 0.3 +
      (100 - metrics.memoryUsage) * 0.3 +
      (1000 - metrics.activeStreams) / 10 * 0.2 +
      (1000 - metrics.queryLatency) / 10 * 0.2
    );
  }

  /**
   * Execute task with exponential backoff retry logic
   */
  private async executeTaskWithRetry(task: Task, agent: AgentRegistration): Promise<void> {
    const maxRetries = task.maxRetries || 3;

    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        const timeout = this.config.taskTimeout;

        // Simulate task execution (replace with actual agent communication)
        await this.executeTaskOnAgent(task, agent, timeout);

        // Task successful
        this.activeTasks.delete(task.id);
        this.emit('task:completed', { taskId: task.id, agentId: agent.agentId });

        // Record success in circuit breaker
        this.circuitBreakers.get(agent.agentId)?.recordSuccess();

        return;

      } catch (error) {
        task.retries = attempt + 1;

        if (attempt < maxRetries) {
          // Calculate backoff delay
          const backoff = Math.min(
            this.config.retryBackoffBase * Math.pow(2, attempt),
            this.config.retryBackoffMax
          );

          console.warn(
            `[AgentCoordinator] Task ${task.id} attempt ${attempt + 1} failed, retrying in ${backoff}ms`,
            error
          );

          await new Promise(resolve => setTimeout(resolve, backoff));
        } else {
          // Max retries exceeded
          console.error(`[AgentCoordinator] Task ${task.id} failed after ${maxRetries} attempts`);
          await this.handleTaskFailure(task, error);

          // Record failure in circuit breaker
          this.circuitBreakers.get(agent.agentId)?.recordFailure();
        }
      }
    }
  }

  /**
   * Execute task on specific agent (placeholder for actual implementation)
   */
  private async executeTaskOnAgent(
    task: Task,
    agent: AgentRegistration,
    timeout: number
  ): Promise<void> {
    // This would be replaced with actual HTTP/gRPC call to agent endpoint
    // For now, simulate execution
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Task timeout')), timeout);

      // Simulate task execution
      setTimeout(() => {
        clearTimeout(timer);
        resolve();
      }, Math.random() * 100);
    });
  }

  /**
   * Handle task failure
   */
  private async handleTaskFailure(task: Task, error: any): Promise<void> {
    this.activeTasks.delete(task.id);

    this.emit('task:failed', {
      taskId: task.id,
      error: error.message,
      retries: task.retries,
    });

    // Could implement dead letter queue here
    console.error(`[AgentCoordinator] Task ${task.id} failed permanently:`, error);
  }

  /**
   * Redistribute task to another agent (failover)
   */
  private async redistributeTask(task: Task): Promise<void> {
    console.log(`[AgentCoordinator] Redistributing task ${task.id}`);

    // Remove region preference to allow any region
    const redistributedTask = { ...task, region: undefined };
    this.insertTaskByPriority(redistributedTask);

    this.emit('task:redistributed', { taskId: task.id });
  }

  /**
   * Failover task when agent is unavailable
   */
  private async failoverTask(task: Task, failedAgentId: string): Promise<void> {
    console.log(`[AgentCoordinator] Failing over task ${task.id} from agent ${failedAgentId}`);

    this.activeTasks.delete(task.id);
    await this.redistributeTask(task);

    this.emit('task:failover', { taskId: task.id, failedAgentId });
  }

  /**
   * Update agent metrics
   */
  updateAgentMetrics(metrics: AgentMetrics): void {
    this.agentMetrics.set(metrics.agentId, {
      ...metrics,
      timestamp: Date.now(),
    });

    // Check if agent health changed
    const previousMetrics = this.agentMetrics.get(metrics.agentId);
    if (previousMetrics && previousMetrics.healthy !== metrics.healthy) {
      this.emit('agent:health-changed', {
        agentId: metrics.agentId,
        healthy: metrics.healthy,
      });
    }
  }

  /**
   * Start health monitoring loop
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
    const now = Date.now();

    for (const [agentId, metrics] of this.agentMetrics.entries()) {
      // Check if metrics are stale (no update in 2x health check interval)
      const staleThreshold = this.config.healthCheckInterval * 2;
      const isStale = now - metrics.timestamp > staleThreshold;

      if (isStale && metrics.healthy) {
        console.warn(`[AgentCoordinator] Agent ${agentId} marked unhealthy (stale metrics)`);

        this.agentMetrics.set(agentId, {
          ...metrics,
          healthy: false,
          timestamp: now,
        });

        this.emit('agent:health-changed', {
          agentId,
          healthy: false,
          reason: 'stale_metrics',
        });
      }
    }
  }

  /**
   * Start task distribution loop
   */
  private startTaskDistribution(): void {
    this.taskDistributionTimer = setInterval(() => {
      this.distributeNextTask().catch(error => {
        console.error('[AgentCoordinator] Error in task distribution:', error);
      });
    }, 100); // Distribute tasks every 100ms
  }

  /**
   * Get coordinator status
   */
  getStatus(): {
    totalAgents: number;
    healthyAgents: number;
    queuedTasks: number;
    activeTasks: number;
    regionDistribution: Record<string, number>;
  } {
    const healthyAgents = Array.from(this.agentMetrics.values()).filter(
      m => m.healthy
    ).length;

    const regionDistribution: Record<string, number> = {};
    for (const agent of this.agents.values()) {
      regionDistribution[agent.region] = (regionDistribution[agent.region] || 0) + 1;
    }

    return {
      totalAgents: this.agents.size,
      healthyAgents,
      queuedTasks: this.taskQueue.length,
      activeTasks: this.activeTasks.size,
      regionDistribution,
    };
  }

  /**
   * Shutdown coordinator gracefully
   */
  async shutdown(): Promise<void> {
    console.log('[AgentCoordinator] Shutting down coordinator...');

    if (this.healthCheckTimer) {
      clearInterval(this.healthCheckTimer);
    }

    if (this.taskDistributionTimer) {
      clearInterval(this.taskDistributionTimer);
    }

    if (this.config.enableClaudeFlowHooks) {
      try {
        // Post-task hook
        await execAsync(
          `npx claude-flow@alpha hooks post-task --task-id "coordinator-shutdown"`
        );
      } catch (error) {
        console.warn('[AgentCoordinator] Error executing post-task hook:', error);
      }
    }

    this.emit('coordinator:shutdown');
  }
}

/**
 * Circuit Breaker for agent fault tolerance
 */
class CircuitBreaker {
  private failures = 0;
  private lastFailureTime = 0;
  private state: 'closed' | 'open' | 'half-open' = 'closed';

  constructor(
    private config: {
      threshold: number;
      timeout: number;
    }
  ) {}

  canExecute(): boolean {
    if (this.state === 'closed') return true;

    if (this.state === 'open') {
      // Check if timeout has passed
      if (Date.now() - this.lastFailureTime > this.config.timeout) {
        this.state = 'half-open';
        return true;
      }
      return false;
    }

    // half-open: allow one request
    return true;
  }

  recordSuccess(): void {
    this.failures = 0;
    this.state = 'closed';
  }

  recordFailure(): void {
    this.failures++;
    this.lastFailureTime = Date.now();

    if (this.failures >= this.config.threshold) {
      this.state = 'open';
    }
  }
}
