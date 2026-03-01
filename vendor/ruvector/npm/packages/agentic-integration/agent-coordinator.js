"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.AgentCoordinator = void 0;
const events_1 = require("events");
const child_process_1 = require("child_process");
const util_1 = require("util");
const execAsync = (0, util_1.promisify)(child_process_1.exec);
class AgentCoordinator extends events_1.EventEmitter {
    constructor(config) {
        super();
        this.config = config;
        this.agents = new Map();
        this.agentMetrics = new Map();
        this.taskQueue = [];
        this.activeTasks = new Map();
        this.regionLoadIndex = new Map();
        this.circuitBreakers = new Map();
        this.initializeCoordinator();
    }
    /**
     * Initialize coordinator with claude-flow hooks
     */
    async initializeCoordinator() {
        console.log('[AgentCoordinator] Initializing coordinator...');
        if (this.config.enableClaudeFlowHooks) {
            try {
                // Pre-task hook for coordination initialization
                await execAsync(`npx claude-flow@alpha hooks pre-task --description "Initialize agent coordinator"`);
                console.log('[AgentCoordinator] Claude-flow pre-task hook executed');
            }
            catch (error) {
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
    async registerAgent(registration) {
        console.log(`[AgentCoordinator] Registering agent: ${registration.agentId} in ${registration.region}`);
        // Check if region has capacity
        const regionAgents = Array.from(this.agents.values()).filter(a => a.region === registration.region);
        if (regionAgents.length >= this.config.maxAgentsPerRegion) {
            throw new Error(`Region ${registration.region} has reached max agent capacity`);
        }
        this.agents.set(registration.agentId, registration);
        // Initialize circuit breaker for agent
        this.circuitBreakers.set(registration.agentId, new CircuitBreaker({
            threshold: this.config.failoverThreshold,
            timeout: this.config.taskTimeout,
        }));
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
    async unregisterAgent(agentId) {
        console.log(`[AgentCoordinator] Unregistering agent: ${agentId}`);
        const agent = this.agents.get(agentId);
        if (!agent) {
            throw new Error(`Agent ${agentId} not found`);
        }
        // Redistribute active tasks
        const agentTasks = Array.from(this.activeTasks.values()).filter(task => task.region === agent.region);
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
    async submitTask(task) {
        const fullTask = {
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
    insertTaskByPriority(task) {
        let insertIndex = this.taskQueue.findIndex(t => t.priority < task.priority);
        if (insertIndex === -1) {
            this.taskQueue.push(task);
        }
        else {
            this.taskQueue.splice(insertIndex, 0, task);
        }
    }
    /**
     * Distribute tasks to agents using configured load balancing strategy
     */
    async distributeNextTask() {
        if (this.taskQueue.length === 0)
            return;
        const task = this.taskQueue.shift();
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
        }
        catch (error) {
            console.error(`[AgentCoordinator] Error distributing task ${task.id}:`, error);
            await this.handleTaskFailure(task, error);
        }
    }
    /**
     * Select best agent for task based on load balancing strategy
     */
    async selectAgent(task) {
        const availableAgents = Array.from(this.agents.values()).filter(agent => {
            const metrics = this.agentMetrics.get(agent.agentId);
            return metrics?.healthy && (!task.region || agent.region === task.region);
        });
        if (availableAgents.length === 0)
            return null;
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
    selectAgentRoundRobin(agents, task) {
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
    selectAgentLeastConnections(agents) {
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
    selectAgentWeighted(agents) {
        const totalCapacity = agents.reduce((sum, a) => sum + a.capacity, 0);
        let random = Math.random() * totalCapacity;
        for (const agent of agents) {
            random -= agent.capacity;
            if (random <= 0)
                return agent;
        }
        return agents[agents.length - 1];
    }
    /**
     * Adaptive load balancing based on real-time metrics
     */
    selectAgentAdaptive(agents) {
        return agents.reduce((best, agent) => {
            const bestMetrics = this.agentMetrics.get(best.agentId);
            const agentMetrics = this.agentMetrics.get(agent.agentId);
            if (!bestMetrics || !agentMetrics)
                return best;
            // Score based on: low CPU, low memory, low streams, low latency
            const bestScore = this.calculateAdaptiveScore(bestMetrics);
            const agentScore = this.calculateAdaptiveScore(agentMetrics);
            return agentScore > bestScore ? agent : best;
        });
    }
    /**
     * Calculate adaptive score for agent selection
     */
    calculateAdaptiveScore(metrics) {
        return ((100 - metrics.cpuUsage) * 0.3 +
            (100 - metrics.memoryUsage) * 0.3 +
            (1000 - metrics.activeStreams) / 10 * 0.2 +
            (1000 - metrics.queryLatency) / 10 * 0.2);
    }
    /**
     * Execute task with exponential backoff retry logic
     */
    async executeTaskWithRetry(task, agent) {
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
            }
            catch (error) {
                task.retries = attempt + 1;
                if (attempt < maxRetries) {
                    // Calculate backoff delay
                    const backoff = Math.min(this.config.retryBackoffBase * Math.pow(2, attempt), this.config.retryBackoffMax);
                    console.warn(`[AgentCoordinator] Task ${task.id} attempt ${attempt + 1} failed, retrying in ${backoff}ms`, error);
                    await new Promise(resolve => setTimeout(resolve, backoff));
                }
                else {
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
    async executeTaskOnAgent(task, agent, timeout) {
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
    async handleTaskFailure(task, error) {
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
    async redistributeTask(task) {
        console.log(`[AgentCoordinator] Redistributing task ${task.id}`);
        // Remove region preference to allow any region
        const redistributedTask = { ...task, region: undefined };
        this.insertTaskByPriority(redistributedTask);
        this.emit('task:redistributed', { taskId: task.id });
    }
    /**
     * Failover task when agent is unavailable
     */
    async failoverTask(task, failedAgentId) {
        console.log(`[AgentCoordinator] Failing over task ${task.id} from agent ${failedAgentId}`);
        this.activeTasks.delete(task.id);
        await this.redistributeTask(task);
        this.emit('task:failover', { taskId: task.id, failedAgentId });
    }
    /**
     * Update agent metrics
     */
    updateAgentMetrics(metrics) {
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
    startHealthMonitoring() {
        this.healthCheckTimer = setInterval(() => {
            this.performHealthChecks();
        }, this.config.healthCheckInterval);
    }
    /**
     * Perform health checks on all agents
     */
    async performHealthChecks() {
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
    startTaskDistribution() {
        this.taskDistributionTimer = setInterval(() => {
            this.distributeNextTask().catch(error => {
                console.error('[AgentCoordinator] Error in task distribution:', error);
            });
        }, 100); // Distribute tasks every 100ms
    }
    /**
     * Get coordinator status
     */
    getStatus() {
        const healthyAgents = Array.from(this.agentMetrics.values()).filter(m => m.healthy).length;
        const regionDistribution = {};
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
    async shutdown() {
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
                await execAsync(`npx claude-flow@alpha hooks post-task --task-id "coordinator-shutdown"`);
            }
            catch (error) {
                console.warn('[AgentCoordinator] Error executing post-task hook:', error);
            }
        }
        this.emit('coordinator:shutdown');
    }
}
exports.AgentCoordinator = AgentCoordinator;
/**
 * Circuit Breaker for agent fault tolerance
 */
class CircuitBreaker {
    constructor(config) {
        this.config = config;
        this.failures = 0;
        this.lastFailureTime = 0;
        this.state = 'closed';
    }
    canExecute() {
        if (this.state === 'closed')
            return true;
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
    recordSuccess() {
        this.failures = 0;
        this.state = 'closed';
    }
    recordFailure() {
        this.failures++;
        this.lastFailureTime = Date.now();
        if (this.failures >= this.config.threshold) {
            this.state = 'open';
        }
    }
}
//# sourceMappingURL=agent-coordinator.js.map