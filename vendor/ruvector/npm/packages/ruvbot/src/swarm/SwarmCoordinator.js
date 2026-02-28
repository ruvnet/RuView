"use strict";
/**
 * SwarmCoordinator - Multi-Agent Swarm Orchestration
 *
 * Provides distributed task coordination using agentic-flow patterns.
 * Supports multiple topologies and consensus protocols.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SwarmCoordinator = exports.WORKER_DEFAULTS = void 0;
exports.createSwarmCoordinator = createSwarmCoordinator;
const uuid_1 = require("uuid");
const events_1 = require("events");
// ============================================================================
// Default Worker Configurations
// ============================================================================
exports.WORKER_DEFAULTS = {
    ultralearn: { type: 'ultralearn', priority: 'normal', concurrency: 2, timeout: 60000, retries: 3, backoff: 'exponential' },
    optimize: { type: 'optimize', priority: 'high', concurrency: 4, timeout: 30000, retries: 2, backoff: 'exponential' },
    consolidate: { type: 'consolidate', priority: 'low', concurrency: 1, timeout: 120000, retries: 1, backoff: 'linear' },
    predict: { type: 'predict', priority: 'normal', concurrency: 2, timeout: 15000, retries: 2, backoff: 'exponential' },
    audit: { type: 'audit', priority: 'critical', concurrency: 1, timeout: 45000, retries: 3, backoff: 'exponential' },
    map: { type: 'map', priority: 'normal', concurrency: 2, timeout: 60000, retries: 2, backoff: 'linear' },
    preload: { type: 'preload', priority: 'low', concurrency: 4, timeout: 10000, retries: 1, backoff: 'linear' },
    deepdive: { type: 'deepdive', priority: 'normal', concurrency: 2, timeout: 90000, retries: 2, backoff: 'exponential' },
    document: { type: 'document', priority: 'normal', concurrency: 2, timeout: 30000, retries: 2, backoff: 'linear' },
    refactor: { type: 'refactor', priority: 'normal', concurrency: 2, timeout: 60000, retries: 2, backoff: 'exponential' },
    benchmark: { type: 'benchmark', priority: 'normal', concurrency: 1, timeout: 120000, retries: 1, backoff: 'linear' },
    testgaps: { type: 'testgaps', priority: 'normal', concurrency: 2, timeout: 45000, retries: 2, backoff: 'linear' },
};
// ============================================================================
// SwarmCoordinator Implementation
// ============================================================================
class SwarmCoordinator extends events_1.EventEmitter {
    constructor(config = {}) {
        super();
        this.agents = new Map();
        this.tasks = new Map();
        this.taskQueue = new Map();
        this.started = false;
        this.config = {
            topology: config.topology ?? 'hierarchical',
            maxAgents: config.maxAgents ?? 8,
            strategy: config.strategy ?? 'specialized',
            consensus: config.consensus ?? 'raft',
            heartbeatInterval: config.heartbeatInterval ?? 5000,
            taskTimeout: config.taskTimeout ?? 60000,
        };
        // Initialize priority queues
        this.taskQueue.set('critical', []);
        this.taskQueue.set('high', []);
        this.taskQueue.set('normal', []);
        this.taskQueue.set('low', []);
    }
    /**
     * Start the swarm coordinator
     */
    async start() {
        if (this.started)
            return;
        // Start heartbeat monitoring
        this.heartbeatTimer = setInterval(() => this.checkHeartbeats(), this.config.heartbeatInterval);
        this.started = true;
        this.emit('started');
    }
    /**
     * Stop the swarm coordinator
     */
    async stop() {
        if (!this.started)
            return;
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = undefined;
        }
        // Mark all agents as offline
        for (const agent of this.agents.values()) {
            agent.status = 'offline';
        }
        this.started = false;
        this.emit('stopped');
    }
    /**
     * Spawn a new worker agent
     */
    async spawnAgent(type) {
        if (this.agents.size >= this.config.maxAgents) {
            throw new Error(`Max agents (${this.config.maxAgents}) reached`);
        }
        const agent = {
            id: (0, uuid_1.v4)(),
            type,
            status: 'idle',
            completedTasks: 0,
            failedTasks: 0,
            lastHeartbeat: new Date(),
        };
        this.agents.set(agent.id, agent);
        this.emit('agent:spawned', agent);
        // Try to assign pending tasks
        await this.processQueue();
        return agent;
    }
    /**
     * Remove an agent
     */
    async removeAgent(agentId) {
        const agent = this.agents.get(agentId);
        if (!agent)
            return false;
        // If agent has a current task, re-queue it
        if (agent.currentTask) {
            const task = this.tasks.get(agent.currentTask);
            if (task && task.status === 'running') {
                task.status = 'pending';
                task.assignedAgent = undefined;
                this.enqueueTask(task);
            }
        }
        this.agents.delete(agentId);
        this.emit('agent:removed', agent);
        return true;
    }
    /**
     * Dispatch a task to the swarm
     */
    async dispatch(options) {
        const workerConfig = exports.WORKER_DEFAULTS[options.worker];
        const priority = options.priority ?? workerConfig.priority;
        const task = {
            id: (0, uuid_1.v4)(),
            worker: options.worker,
            type: options.task.type,
            content: options.task.content,
            priority,
            status: 'pending',
            createdAt: new Date(),
        };
        this.tasks.set(task.id, task);
        this.enqueueTask(task);
        this.emit('task:created', task);
        // Try to assign immediately
        await this.processQueue();
        return task;
    }
    /**
     * Wait for a task to complete
     */
    async waitForTask(taskId, timeout) {
        const effectiveTimeout = timeout ?? this.config.taskTimeout;
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                reject(new Error(`Task ${taskId} timed out`));
            }, effectiveTimeout);
            const checkTask = () => {
                const task = this.tasks.get(taskId);
                if (!task) {
                    clearTimeout(timer);
                    reject(new Error(`Task ${taskId} not found`));
                    return;
                }
                if (task.status === 'completed' || task.status === 'failed') {
                    clearTimeout(timer);
                    resolve(task);
                    return;
                }
                // Check again soon
                setTimeout(checkTask, 100);
            };
            checkTask();
        });
    }
    /**
     * Complete a task (called by worker)
     */
    completeTask(taskId, result, error) {
        const task = this.tasks.get(taskId);
        if (!task)
            return;
        task.completedAt = new Date();
        if (error) {
            task.status = 'failed';
            task.error = error;
        }
        else {
            task.status = 'completed';
            task.result = result;
        }
        // Update agent stats
        if (task.assignedAgent) {
            const agent = this.agents.get(task.assignedAgent);
            if (agent) {
                agent.status = 'idle';
                agent.currentTask = undefined;
                if (error) {
                    agent.failedTasks++;
                }
                else {
                    agent.completedTasks++;
                }
            }
        }
        this.emit(error ? 'task:failed' : 'task:completed', task);
        // Process queue for next task
        this.processQueue();
    }
    /**
     * Get swarm status
     */
    getStatus() {
        let idleAgents = 0;
        let busyAgents = 0;
        let pendingTasks = 0;
        let runningTasks = 0;
        let completedTasks = 0;
        let failedTasks = 0;
        for (const agent of this.agents.values()) {
            if (agent.status === 'idle')
                idleAgents++;
            if (agent.status === 'busy')
                busyAgents++;
        }
        for (const task of this.tasks.values()) {
            switch (task.status) {
                case 'pending':
                    pendingTasks++;
                    break;
                case 'running':
                    runningTasks++;
                    break;
                case 'completed':
                    completedTasks++;
                    break;
                case 'failed':
                    failedTasks++;
                    break;
            }
        }
        return {
            topology: this.config.topology,
            consensus: this.config.consensus,
            agentCount: this.agents.size,
            maxAgents: this.config.maxAgents,
            idleAgents,
            busyAgents,
            pendingTasks,
            runningTasks,
            completedTasks,
            failedTasks,
        };
    }
    /**
     * Get all agents
     */
    getAgents() {
        return Array.from(this.agents.values());
    }
    /**
     * Get agent by ID
     */
    getAgent(agentId) {
        return this.agents.get(agentId);
    }
    /**
     * Get all tasks
     */
    getTasks() {
        return Array.from(this.tasks.values());
    }
    /**
     * Get task by ID
     */
    getTask(taskId) {
        return this.tasks.get(taskId);
    }
    /**
     * Update agent heartbeat
     */
    heartbeat(agentId) {
        const agent = this.agents.get(agentId);
        if (agent) {
            agent.lastHeartbeat = new Date();
        }
    }
    // ==========================================================================
    // Private Methods
    // ==========================================================================
    enqueueTask(task) {
        const queue = this.taskQueue.get(task.priority);
        if (queue) {
            queue.push(task);
        }
    }
    dequeueTask() {
        // Priority order: critical > high > normal > low
        const priorities = ['critical', 'high', 'normal', 'low'];
        for (const priority of priorities) {
            const queue = this.taskQueue.get(priority);
            if (queue && queue.length > 0) {
                return queue.shift();
            }
        }
        return undefined;
    }
    async processQueue() {
        // Find idle agents matching pending tasks
        for (const agent of this.agents.values()) {
            if (agent.status !== 'idle')
                continue;
            // Find a matching task
            const task = this.findTaskForAgent(agent);
            if (!task)
                continue;
            // Assign task
            await this.assignTask(task, agent);
        }
    }
    findTaskForAgent(agent) {
        const priorities = ['critical', 'high', 'normal', 'low'];
        for (const priority of priorities) {
            const queue = this.taskQueue.get(priority);
            if (!queue)
                continue;
            // Find task matching agent type (for specialized strategy)
            // or any task (for balanced strategy)
            const taskIndex = queue.findIndex(task => {
                if (this.config.strategy === 'specialized') {
                    return task.worker === agent.type;
                }
                return true;
            });
            if (taskIndex >= 0) {
                return queue.splice(taskIndex, 1)[0];
            }
        }
        return undefined;
    }
    async assignTask(task, agent) {
        task.status = 'running';
        task.assignedAgent = agent.id;
        task.startedAt = new Date();
        agent.status = 'busy';
        agent.currentTask = task.id;
        this.emit('task:assigned', { task, agent });
        // Set timeout for task
        const workerConfig = exports.WORKER_DEFAULTS[task.worker];
        setTimeout(() => {
            const currentTask = this.tasks.get(task.id);
            if (currentTask && currentTask.status === 'running') {
                this.completeTask(task.id, undefined, 'Task timed out');
            }
        }, workerConfig.timeout);
    }
    checkHeartbeats() {
        const now = Date.now();
        const threshold = this.config.heartbeatInterval * 3; // 3 missed heartbeats
        for (const agent of this.agents.values()) {
            if (agent.status === 'offline')
                continue;
            const lastHeartbeat = agent.lastHeartbeat.getTime();
            if (now - lastHeartbeat > threshold) {
                agent.status = 'offline';
                this.emit('agent:offline', agent);
                // Re-queue any running task
                if (agent.currentTask) {
                    const task = this.tasks.get(agent.currentTask);
                    if (task && task.status === 'running') {
                        task.status = 'pending';
                        task.assignedAgent = undefined;
                        this.enqueueTask(task);
                    }
                }
            }
        }
    }
}
exports.SwarmCoordinator = SwarmCoordinator;
// ============================================================================
// Factory Function
// ============================================================================
function createSwarmCoordinator(config) {
    return new SwarmCoordinator(config);
}
exports.default = SwarmCoordinator;
//# sourceMappingURL=SwarmCoordinator.js.map