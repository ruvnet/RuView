#!/usr/bin/env node
/**
 * Edge-Net Agent System
 *
 * Distributed AI agent execution across the Edge-Net collective.
 * Spawn agents, create worker pools, and orchestrate multi-agent workflows.
 */

import { EventEmitter } from 'events';
import { randomBytes, createHash } from 'crypto';

// Agent types and their capabilities
export const AGENT_TYPES = {
    researcher: {
        name: 'Researcher',
        capabilities: ['search', 'analyze', 'summarize', 'extract'],
        baseRuv: 10,
        description: 'Analyzes and researches information',
    },
    coder: {
        name: 'Coder',
        capabilities: ['code', 'refactor', 'debug', 'test'],
        baseRuv: 15,
        description: 'Writes and improves code',
    },
    reviewer: {
        name: 'Reviewer',
        capabilities: ['review', 'audit', 'validate', 'suggest'],
        baseRuv: 12,
        description: 'Reviews code and provides feedback',
    },
    tester: {
        name: 'Tester',
        capabilities: ['test', 'benchmark', 'validate', 'report'],
        baseRuv: 10,
        description: 'Tests and validates implementations',
    },
    analyst: {
        name: 'Analyst',
        capabilities: ['analyze', 'metrics', 'report', 'visualize'],
        baseRuv: 8,
        description: 'Analyzes data and generates reports',
    },
    optimizer: {
        name: 'Optimizer',
        capabilities: ['optimize', 'profile', 'benchmark', 'improve'],
        baseRuv: 15,
        description: 'Optimizes performance and efficiency',
    },
    coordinator: {
        name: 'Coordinator',
        capabilities: ['orchestrate', 'route', 'schedule', 'monitor'],
        baseRuv: 20,
        description: 'Coordinates multi-agent workflows',
    },
    embedder: {
        name: 'Embedder',
        capabilities: ['embed', 'vectorize', 'similarity', 'search'],
        baseRuv: 5,
        description: 'Generates embeddings and vector operations',
    },
};

// Task status enum
export const TaskStatus = {
    PENDING: 'pending',
    QUEUED: 'queued',
    ASSIGNED: 'assigned',
    RUNNING: 'running',
    COMPLETED: 'completed',
    FAILED: 'failed',
    CANCELLED: 'cancelled',
};

/**
 * Distributed Agent
 *
 * Represents an AI agent running on the Edge-Net network.
 */
export class DistributedAgent extends EventEmitter {
    constructor(options) {
        super();
        this.id = `agent-${randomBytes(8).toString('hex')}`;
        this.type = options.type || 'researcher';
        this.task = options.task;
        this.config = AGENT_TYPES[this.type] || AGENT_TYPES.researcher;
        this.maxRuv = options.maxRuv || this.config.baseRuv;
        this.priority = options.priority || 'medium';
        this.timeout = options.timeout || 300000; // 5 min default

        this.status = TaskStatus.PENDING;
        this.assignedNode = null;
        this.progress = 0;
        this.result = null;
        this.error = null;
        this.startTime = null;
        this.endTime = null;
        this.ruvSpent = 0;

        this.subtasks = [];
        this.logs = [];
    }

    /**
     * Get agent info
     */
    getInfo() {
        return {
            id: this.id,
            type: this.type,
            task: this.task,
            status: this.status,
            progress: this.progress,
            assignedNode: this.assignedNode,
            maxRuv: this.maxRuv,
            ruvSpent: this.ruvSpent,
            startTime: this.startTime,
            endTime: this.endTime,
            duration: this.endTime && this.startTime
                ? this.endTime - this.startTime
                : null,
        };
    }

    /**
     * Update agent progress
     */
    updateProgress(progress, message) {
        this.progress = Math.min(100, Math.max(0, progress));
        this.log(`Progress: ${this.progress}% - ${message}`);
        this.emit('progress', { progress: this.progress, message });
    }

    /**
     * Log message
     */
    log(message) {
        const entry = {
            timestamp: Date.now(),
            message,
        };
        this.logs.push(entry);
        this.emit('log', entry);
    }

    /**
     * Mark as completed
     */
    complete(result) {
        this.status = TaskStatus.COMPLETED;
        this.result = result;
        this.progress = 100;
        this.endTime = Date.now();
        this.log('Agent completed successfully');
        this.emit('complete', result);
    }

    /**
     * Mark as failed
     */
    fail(error) {
        this.status = TaskStatus.FAILED;
        this.error = error;
        this.endTime = Date.now();
        this.log(`Agent failed: ${error}`);
        this.emit('error', error);
    }

    /**
     * Cancel the agent
     */
    cancel() {
        this.status = TaskStatus.CANCELLED;
        this.endTime = Date.now();
        this.log('Agent cancelled');
        this.emit('cancelled');
    }
}

/**
 * Agent Spawner
 *
 * Spawns and manages distributed agents across the Edge-Net network.
 */
export class AgentSpawner extends EventEmitter {
    constructor(networkManager, options = {}) {
        super();
        this.network = networkManager;
        this.agents = new Map();
        this.pendingQueue = [];
        this.maxConcurrent = options.maxConcurrent || 10;
        this.defaultTimeout = options.defaultTimeout || 300000;

        // Agent routing table (learned from outcomes)
        this.routingTable = new Map();

        // Stats
        this.stats = {
            totalSpawned: 0,
            completed: 0,
            failed: 0,
            totalRuvSpent: 0,
        };
    }

    /**
     * Spawn a new agent on the network
     */
    async spawn(options) {
        const agent = new DistributedAgent({
            ...options,
            timeout: options.timeout || this.defaultTimeout,
        });

        this.agents.set(agent.id, agent);
        this.stats.totalSpawned++;

        agent.log(`Agent spawned: ${agent.type} - ${agent.task}`);
        agent.status = TaskStatus.QUEUED;

        // Find best node for this agent type
        const targetNode = await this.findBestNode(agent);

        if (targetNode) {
            await this.assignToNode(agent, targetNode);
        } else {
            // Queue for later assignment
            this.pendingQueue.push(agent);
            agent.log('Queued - waiting for available node');
        }

        this.emit('agent-spawned', agent);
        return agent;
    }

    /**
     * Find the best node for an agent based on capabilities and load
     */
    async findBestNode(agent) {
        if (!this.network) return null;

        const peers = this.network.getPeerList ?
            this.network.getPeerList() :
            Array.from(this.network.peers?.values() || []);

        if (peers.length === 0) return null;

        // Score each peer based on:
        // 1. Capability match
        // 2. Current load
        // 3. Historical performance
        // 4. Latency
        const scoredPeers = peers.map(peer => {
            let score = 50; // Base score

            // Check capabilities
            const peerCaps = peer.capabilities || [];
            const requiredCaps = agent.config.capabilities;
            const capMatch = requiredCaps.filter(c => peerCaps.includes(c)).length;
            score += capMatch * 10;

            // Check load (lower is better)
            const load = peer.load || 0;
            score -= load * 20;

            // Check historical performance
            const history = this.routingTable.get(`${peer.piKey || peer.id}-${agent.type}`);
            if (history) {
                score += history.successRate * 30;
                score -= history.avgLatency / 1000; // Penalize high latency
            }

            return { peer, score };
        });

        // Sort by score (highest first)
        scoredPeers.sort((a, b) => b.score - a.score);

        return scoredPeers[0]?.peer || null;
    }

    /**
     * Assign agent to a specific node
     */
    async assignToNode(agent, node) {
        agent.status = TaskStatus.ASSIGNED;
        agent.assignedNode = node.piKey || node.id;
        agent.startTime = Date.now();
        agent.log(`Assigned to node: ${agent.assignedNode.slice(0, 12)}...`);

        // Send task to node via network
        if (this.network?.sendToPeer) {
            await this.network.sendToPeer(agent.assignedNode, {
                type: 'agent_task',
                agentId: agent.id,
                agentType: agent.type,
                task: agent.task,
                maxRuv: agent.maxRuv,
                timeout: agent.timeout,
            });
        }

        agent.status = TaskStatus.RUNNING;
        this.emit('agent-assigned', { agent, node });

        // Set timeout
        setTimeout(() => {
            if (agent.status === TaskStatus.RUNNING) {
                agent.fail('Timeout exceeded');
            }
        }, agent.timeout);
    }

    /**
     * Handle task result from network
     */
    handleResult(agentId, result) {
        const agent = this.agents.get(agentId);
        if (!agent) return;

        if (result.success) {
            agent.complete(result.data);
            this.stats.completed++;
            this.updateRoutingTable(agent, true, result.latency);
        } else {
            agent.fail(result.error);
            this.stats.failed++;
            this.updateRoutingTable(agent, false, result.latency);
        }

        agent.ruvSpent = result.ruvSpent || agent.config.baseRuv;
        this.stats.totalRuvSpent += agent.ruvSpent;
    }

    /**
     * Update routing table with outcome
     */
    updateRoutingTable(agent, success, latency) {
        const key = `${agent.assignedNode}-${agent.type}`;
        const existing = this.routingTable.get(key) || {
            attempts: 0,
            successes: 0,
            totalLatency: 0,
        };

        existing.attempts++;
        if (success) existing.successes++;
        existing.totalLatency += latency || 0;
        existing.successRate = existing.successes / existing.attempts;
        existing.avgLatency = existing.totalLatency / existing.attempts;

        this.routingTable.set(key, existing);
    }

    /**
     * Get agent by ID
     */
    getAgent(agentId) {
        return this.agents.get(agentId);
    }

    /**
     * List all agents
     */
    listAgents(filter = {}) {
        let agents = Array.from(this.agents.values());

        if (filter.status) {
            agents = agents.filter(a => a.status === filter.status);
        }
        if (filter.type) {
            agents = agents.filter(a => a.type === filter.type);
        }

        return agents.map(a => a.getInfo());
    }

    /**
     * Get spawner stats
     */
    getStats() {
        return {
            ...this.stats,
            activeAgents: Array.from(this.agents.values())
                .filter(a => a.status === TaskStatus.RUNNING).length,
            queuedAgents: this.pendingQueue.length,
            successRate: this.stats.completed /
                (this.stats.completed + this.stats.failed) || 0,
        };
    }
}

/**
 * Worker Pool
 *
 * Manages a pool of distributed workers for parallel task execution.
 */
export class WorkerPool extends EventEmitter {
    constructor(networkManager, options = {}) {
        super();
        this.id = `pool-${randomBytes(6).toString('hex')}`;
        this.network = networkManager;
        this.size = options.size || 5;
        this.capabilities = options.capabilities || ['compute', 'embed'];
        this.maxTasksPerWorker = options.maxTasksPerWorker || 10;

        this.workers = new Map();
        this.taskQueue = [];
        this.activeTasks = new Map();
        this.results = new Map();

        this.status = 'initializing';
        this.stats = {
            tasksCompleted: 0,
            tasksFailed: 0,
            totalProcessingTime: 0,
        };
    }

    /**
     * Initialize the worker pool
     */
    async initialize() {
        this.status = 'recruiting';
        this.emit('status', 'Recruiting workers...');

        // Find available workers from network
        const peers = this.network?.getPeerList?.() ||
            Array.from(this.network?.peers?.values() || []);

        // Filter peers by capabilities
        const eligiblePeers = peers.filter(peer => {
            const peerCaps = peer.capabilities || [];
            return this.capabilities.some(c => peerCaps.includes(c));
        });

        // Recruit up to pool size
        const recruited = eligiblePeers.slice(0, this.size);

        for (const peer of recruited) {
            this.workers.set(peer.piKey || peer.id, {
                id: peer.piKey || peer.id,
                peer,
                status: 'idle',
                currentTasks: 0,
                completedTasks: 0,
                lastSeen: Date.now(),
            });
        }

        // If not enough real workers, create virtual workers for local execution
        while (this.workers.size < this.size) {
            const virtualId = `virtual-${randomBytes(4).toString('hex')}`;
            this.workers.set(virtualId, {
                id: virtualId,
                peer: null,
                status: 'idle',
                currentTasks: 0,
                completedTasks: 0,
                isVirtual: true,
            });
        }

        this.status = 'ready';
        this.emit('ready', {
            poolId: this.id,
            workers: this.workers.size,
            realWorkers: Array.from(this.workers.values())
                .filter(w => !w.isVirtual).length,
        });

        return this;
    }

    /**
     * Execute tasks in parallel across workers
     */
    async execute(options) {
        const {
            task,
            data,
            strategy = 'parallel',
            chunkSize = null,
        } = options;

        const batchId = `batch-${randomBytes(6).toString('hex')}`;
        const startTime = Date.now();

        // Split data into chunks for workers
        let chunks;
        if (Array.isArray(data)) {
            const size = chunkSize || Math.ceil(data.length / this.workers.size);
            chunks = [];
            for (let i = 0; i < data.length; i += size) {
                chunks.push(data.slice(i, i + size));
            }
        } else {
            chunks = [data];
        }

        this.emit('batch-start', { batchId, chunks: chunks.length });

        // Assign chunks to workers
        const promises = chunks.map((chunk, index) =>
            this.assignTask({
                batchId,
                index,
                task,
                data: chunk,
            })
        );

        // Wait for all or handle based on strategy
        let results;
        if (strategy === 'parallel') {
            results = await Promise.all(promises);
        } else if (strategy === 'race') {
            results = [await Promise.race(promises)];
        } else {
            // Sequential
            results = [];
            for (const promise of promises) {
                results.push(await promise);
            }
        }

        const endTime = Date.now();
        this.stats.totalProcessingTime += endTime - startTime;

        this.emit('batch-complete', {
            batchId,
            duration: endTime - startTime,
            results: results.length,
        });

        // Flatten results if array
        return Array.isArray(data) ? results.flat() : results[0];
    }

    /**
     * Assign a single task to an available worker
     */
    async assignTask(taskInfo) {
        const taskId = `task-${randomBytes(6).toString('hex')}`;

        // Find idle worker
        const worker = this.findIdleWorker();
        if (!worker) {
            // Queue task
            return new Promise((resolve, reject) => {
                this.taskQueue.push({ taskInfo, resolve, reject });
            });
        }

        worker.status = 'busy';
        worker.currentTasks++;

        this.activeTasks.set(taskId, {
            ...taskInfo,
            workerId: worker.id,
            startTime: Date.now(),
        });

        try {
            // Execute on worker
            const result = await this.executeOnWorker(worker, taskInfo);

            worker.completedTasks++;
            this.stats.tasksCompleted++;
            this.results.set(taskId, result);

            return result;
        } catch (error) {
            this.stats.tasksFailed++;
            throw error;
        } finally {
            worker.currentTasks--;
            if (worker.currentTasks === 0) {
                worker.status = 'idle';
            }
            this.activeTasks.delete(taskId);

            // Process queued task if any
            if (this.taskQueue.length > 0) {
                const { taskInfo, resolve, reject } = this.taskQueue.shift();
                this.assignTask(taskInfo).then(resolve).catch(reject);
            }
        }
    }

    /**
     * Find an idle worker
     */
    findIdleWorker() {
        for (const worker of this.workers.values()) {
            if (worker.status === 'idle' ||
                worker.currentTasks < this.maxTasksPerWorker) {
                return worker;
            }
        }
        return null;
    }

    /**
     * Execute task on a specific worker
     */
    async executeOnWorker(worker, taskInfo) {
        if (worker.isVirtual) {
            // Local execution for virtual workers
            return this.executeLocally(taskInfo);
        }

        // Send to remote worker via network
        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Worker timeout'));
            }, 60000);

            // Send task
            if (this.network?.sendToPeer) {
                this.network.sendToPeer(worker.id, {
                    type: 'worker_task',
                    poolId: this.id,
                    task: taskInfo.task,
                    data: taskInfo.data,
                });
            }

            // Listen for result
            const handler = (msg) => {
                if (msg.poolId === this.id && msg.batchId === taskInfo.batchId) {
                    clearTimeout(timeout);
                    this.network?.off?.('worker_result', handler);
                    resolve(msg.result);
                }
            };

            this.network?.on?.('worker_result', handler);

            // Fallback to local if no response
            setTimeout(() => {
                clearTimeout(timeout);
                this.executeLocally(taskInfo).then(resolve).catch(reject);
            }, 5000);
        });
    }

    /**
     * Execute task locally (for virtual workers or fallback)
     */
    async executeLocally(taskInfo) {
        const { task, data } = taskInfo;

        // Simple local execution based on task type
        switch (task) {
            case 'embed':
                // Simulate embedding
                return Array.isArray(data)
                    ? data.map(() => new Array(384).fill(0).map(() => Math.random()))
                    : new Array(384).fill(0).map(() => Math.random());

            case 'process':
                return Array.isArray(data)
                    ? data.map(item => ({ processed: true, item }))
                    : { processed: true, data };

            case 'analyze':
                return {
                    analyzed: true,
                    itemCount: Array.isArray(data) ? data.length : 1,
                    timestamp: Date.now(),
                };

            default:
                return { task, data, executed: true };
        }
    }

    /**
     * Get pool status
     */
    getStatus() {
        const workers = Array.from(this.workers.values());
        return {
            poolId: this.id,
            status: this.status,
            totalWorkers: workers.length,
            idleWorkers: workers.filter(w => w.status === 'idle').length,
            busyWorkers: workers.filter(w => w.status === 'busy').length,
            virtualWorkers: workers.filter(w => w.isVirtual).length,
            queuedTasks: this.taskQueue.length,
            activeTasks: this.activeTasks.size,
            stats: this.stats,
        };
    }

    /**
     * Shutdown the pool
     */
    async shutdown() {
        this.status = 'shutting_down';

        // Wait for active tasks
        while (this.activeTasks.size > 0) {
            await new Promise(r => setTimeout(r, 100));
        }

        // Clear workers
        this.workers.clear();
        this.status = 'shutdown';
        this.emit('shutdown');
    }
}

/**
 * Task Orchestrator
 *
 * Orchestrates multi-agent workflows and complex task pipelines.
 */
export class TaskOrchestrator extends EventEmitter {
    constructor(agentSpawner, workerPool, options = {}) {
        super();
        this.spawner = agentSpawner;
        this.pool = workerPool;
        this.workflows = new Map();
        this.maxConcurrentWorkflows = options.maxConcurrentWorkflows || 5;
    }

    /**
     * Create a workflow
     */
    createWorkflow(name, steps) {
        const workflow = {
            id: `wf-${randomBytes(6).toString('hex')}`,
            name,
            steps,
            status: 'created',
            currentStep: 0,
            results: [],
            startTime: null,
            endTime: null,
        };

        this.workflows.set(workflow.id, workflow);
        return workflow;
    }

    /**
     * Execute a workflow
     */
    async executeWorkflow(workflowId, input = {}) {
        const workflow = this.workflows.get(workflowId);
        if (!workflow) throw new Error('Workflow not found');

        workflow.status = 'running';
        workflow.startTime = Date.now();
        workflow.input = input;

        this.emit('workflow-start', { workflowId, name: workflow.name });

        try {
            let context = { ...input };

            for (let i = 0; i < workflow.steps.length; i++) {
                workflow.currentStep = i;
                const step = workflow.steps[i];

                this.emit('step-start', {
                    workflowId,
                    step: i,
                    type: step.type,
                    name: step.name,
                });

                const result = await this.executeStep(step, context);
                workflow.results.push(result);

                // Pass result to next step
                context = { ...context, [step.name || `step${i}`]: result };

                this.emit('step-complete', {
                    workflowId,
                    step: i,
                    result,
                });
            }

            workflow.status = 'completed';
            workflow.endTime = Date.now();

            this.emit('workflow-complete', {
                workflowId,
                duration: workflow.endTime - workflow.startTime,
                results: workflow.results,
            });

            return {
                success: true,
                results: workflow.results,
                context,
            };

        } catch (error) {
            workflow.status = 'failed';
            workflow.endTime = Date.now();
            workflow.error = error.message;

            this.emit('workflow-failed', { workflowId, error: error.message });

            return {
                success: false,
                error: error.message,
                failedStep: workflow.currentStep,
            };
        }
    }

    /**
     * Execute a single workflow step
     */
    async executeStep(step, context) {
        switch (step.type) {
            case 'agent':
                return this.executeAgentStep(step, context);

            case 'parallel':
                return this.executeParallelStep(step, context);

            case 'pool':
                return this.executePoolStep(step, context);

            case 'condition':
                return this.executeConditionStep(step, context);

            case 'transform':
                return this.executeTransformStep(step, context);

            default:
                throw new Error(`Unknown step type: ${step.type}`);
        }
    }

    /**
     * Execute an agent step
     */
    async executeAgentStep(step, context) {
        const task = typeof step.task === 'function'
            ? step.task(context)
            : step.task;

        const agent = await this.spawner.spawn({
            type: step.agentType || 'researcher',
            task,
            maxRuv: step.maxRuv,
            priority: step.priority,
        });

        return new Promise((resolve, reject) => {
            agent.on('complete', resolve);
            agent.on('error', reject);

            // Simulate completion for now
            setTimeout(() => {
                agent.complete({
                    task,
                    result: `Completed: ${task}`,
                    context,
                });
            }, 1000);
        });
    }

    /**
     * Execute parallel agents
     */
    async executeParallelStep(step, context) {
        const promises = step.agents.map(agentConfig =>
            this.executeAgentStep(agentConfig, context)
        );

        return Promise.all(promises);
    }

    /**
     * Execute worker pool step
     */
    async executePoolStep(step, context) {
        const data = typeof step.data === 'function'
            ? step.data(context)
            : step.data || context.data;

        return this.pool.execute({
            task: step.task,
            data,
            strategy: step.strategy || 'parallel',
        });
    }

    /**
     * Execute conditional step
     */
    async executeConditionStep(step, context) {
        const condition = typeof step.condition === 'function'
            ? step.condition(context)
            : step.condition;

        if (condition) {
            return this.executeStep(step.then, context);
        } else if (step.else) {
            return this.executeStep(step.else, context);
        }
        return null;
    }

    /**
     * Execute transform step
     */
    async executeTransformStep(step, context) {
        return step.transform(context);
    }

    /**
     * Get workflow status
     */
    getWorkflowStatus(workflowId) {
        const workflow = this.workflows.get(workflowId);
        if (!workflow) return null;

        return {
            id: workflow.id,
            name: workflow.name,
            status: workflow.status,
            currentStep: workflow.currentStep,
            totalSteps: workflow.steps.length,
            progress: (workflow.currentStep / workflow.steps.length) * 100,
            startTime: workflow.startTime,
            endTime: workflow.endTime,
            duration: workflow.endTime && workflow.startTime
                ? workflow.endTime - workflow.startTime
                : null,
        };
    }

    /**
     * List all workflows
     */
    listWorkflows() {
        return Array.from(this.workflows.values()).map(w => ({
            id: w.id,
            name: w.name,
            status: w.status,
            steps: w.steps.length,
        }));
    }
}

// Export default instances
export default {
    AGENT_TYPES,
    TaskStatus,
    DistributedAgent,
    AgentSpawner,
    WorkerPool,
    TaskOrchestrator,
};
