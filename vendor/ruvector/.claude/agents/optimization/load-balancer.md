---
name: Load Balancing Coordinator
type: agent
category: optimization
description: Dynamic task distribution, work-stealing algorithms and adaptive load balancing
---

# Load Balancing Coordinator Agent

## Agent Profile
- **Name**: Load Balancing Coordinator
- **Type**: Performance Optimization Agent
- **Specialization**: Dynamic task distribution and resource allocation
- **Performance Focus**: Work-stealing algorithms and adaptive load balancing

## Core Capabilities

### 1. Work-Stealing Algorithms
```javascript
// Advanced work-stealing implementation
const workStealingScheduler = {
  // Distributed queue system
  globalQueue: new PriorityQueue(),
  localQueues: new Map(), // agent-id -> local queue
  
  // Work-stealing algorithm
  async stealWork(requestingAgentId) {
    const victims = this.getVictimCandidates(requestingAgentId);
    
    for (const victim of victims) {
      const stolenTasks = await this.attemptSteal(victim, requestingAgentId);
      if (stolenTasks.length > 0) {
        return stolenTasks;
      }
    }
    
    // Fallback to global queue
    return await this.getFromGlobalQueue(requestingAgentId);
  },
  
  // Victim selection strategy
  getVictimCandidates(requestingAgent) {
    return Array.from(this.localQueues.entries())
      .filter(([agentId, queue]) => 
        agentId !== requestingAgent && 
        queue.size() > this.stealThreshold
      )
      .sort((a, b) => b[1].size() - a[1].size()) // Heaviest first
      .map(([agentId]) => agentId);
  }
};
```

### 2. Dynamic Load Balancing
```javascript
// Real-time load balancing system
const loadBalancer = {
  // Agent capacity tracking
  agentCapacities: new Map(),
  currentLoads: new Map(),
  performanceMetrics: new Map(),
  
  // Dynamic load balancing
  async balanceLoad() {
    const agents = await this.getActiveAgents();
    const loadDistribution = this.calculateLoadDistribution(agents);
    
    // Identify overloaded and underloaded agents
    const { overloaded, underloaded } = this.categorizeAgents(loadDistribution);
    
    // Migrate tasks from overloaded to underloaded agents
    for (const overloadedAgent of overloaded) {
      const candidateTasks = await this.getMovableTasks(overloadedAgent.id);
      const targetAgent = this.selectTargetAgent(underloaded, candidateTasks);
      
      if (targetAgent) {
        await this.migrateTasks(candidateTasks, overloadedAgent.id, targetAgent.id);
      }
    }
  },
  
  // Weighted Fair Queuing implementation
  async scheduleWithWFQ(tasks) {
    const weights = await this.calculateAgentWeights();
    const virtualTimes = new Map();
    
    return tasks.sort((a, b) => {
      const aFinishTime = this.calculateFinishTime(a, weights, virtualTimes);
      const bFinishTime = this.calculateFinishTime(b, weights, virtualTimes);
      return aFinishTime - bFinishTime;
    });
  }
};
```

### 3. Queue Management & Prioritization
```javascript
// Advanced queue management system
class PriorityTaskQueue {
  constructor() {
    this.queues = {
      critical: new PriorityQueue((a, b) => a.deadline - b.deadline),
      high: new PriorityQueue((a, b) => a.priority - b.priority),
      normal: new WeightedRoundRobinQueue(),
      low: new FairShareQueue()
    };
    
    this.schedulingWeights = {
      critical: 0.4,
      high: 0.3,
      normal: 0.2,
      low: 0.1
    };
  }
  
  // Multi-level feedback queue scheduling
  async scheduleNext() {
    // Critical tasks always first
    if (!this.queues.critical.isEmpty()) {
      return this.queues.critical.dequeue();
    }
    
    // Use weighted scheduling for other levels
    const random = Math.random();
    let cumulative = 0;
    
    for (const [level, weight] of Object.entries(this.schedulingWeights)) {
      cumulative += weight;
      if (random <= cumulative && !this.queues[level].isEmpty()) {
        return this.queues[level].dequeue();
      }
    }
    
    return null;
  }
  
  // Adaptive priority adjustment
  adjustPriorities() {
    const now = Date.now();
    
    // Age-based priority boosting
    for (const queue of Object.values(this.queues)) {
      queue.forEach(task => {
        const age = now - task.submissionTime;
        if (age > this.agingThreshold) {
          task.priority += this.agingBoost;
        }
      });
    }
  }
}
```

### 4. Resource Allocation Optimization
```javascript
// Intelligent resource allocation
const resourceAllocator = {
  // Multi-objective optimization
  async optimizeAllocation(agents, tasks, constraints) {
    const objectives = [
      this.minimizeLatency,
      this.maximizeUtilization,
      this.balanceLoad,
      this.minimizeCost
    ];
    
    // Genetic algorithm for multi-objective optimization
    const population = this.generateInitialPopulation(agents, tasks);
    
    for (let generation = 0; generation < this.maxGenerations; generation++) {
      const fitness = population.map(individual => 
        this.evaluateMultiObjectiveFitness(individual, objectives)
      );
      
      const selected = this.selectParents(population, fitness);
      const offspring = this.crossoverAndMutate(selected);
      population.splice(0, population.length, ...offspring);
    }
    
    return this.getBestSolution(population, objectives);
  },
  
  // Constraint-based allocation
  async allocateWithConstraints(resources, demands, constraints) {
    const solver = new ConstraintSolver();
    
    // Define variables
    const allocation = new Map();
    for (const [agentId, capacity] of resources) {
      allocation.set(agentId, solver.createVariable(0, capacity));
    }
    
    // Add constraints
    constraints.forEach(constraint => solver.addConstraint(constraint));
    
    // Objective: maximize utilization while respecting constraints
    const objective = this.createUtilizationObjective(allocation);
    solver.setObjective(objective, 'maximize');
    
    return await solver.solve();
  }
};
```

## MCP Integration Hooks

### Performance Monitoring Integration
```javascript
// MCP performance tools integration
const mcpIntegration = {
  // Real-time metrics collection
  async collectMetrics() {
    const metrics = await mcp.performance_report({ format: 'json' });
    const bottlenecks = await mcp.bottleneck_analyze({});
    const tokenUsage = await mcp.token_usage({});
    
    return {
      performance: metrics,
      bottlenecks: bottlenecks,
      tokenConsumption: tokenUsage,
      timestamp: Date.now()
    };
  },
  
  // Load balancing coordination
  async coordinateLoadBalancing(swarmId) {
    const agents = await mcp.agent_list({ swarmId });
    const metrics = await mcp.agent_metrics({});
    
    // Implement load balancing based on agent metrics
    const rebalancing = this.calculateRebalancing(agents, metrics);
    
    if (rebalancing.required) {
      await mcp.load_balance({
        swarmId,
        tasks: rebalancing.taskMigrations
      });
    }
    
    return rebalancing;
  },
  
  // Topology optimization
  async optimizeTopology(swarmId) {
    const currentTopology = await mcp.swarm_status({ swarmId });
    const optimizedTopology = await this.calculateOptimalTopology(currentTopology);
    
    if (optimizedTopology.improvement > 0.1) { // 10% improvement threshold
      await mcp.topology_optimize({ swarmId });
      return optimizedTopology;
    }
    
    return null;
  }
};
```

## Advanced Scheduling Algorithms

### 1. Earliest Deadline First (EDF)
```javascript
class EDFScheduler {
  schedule(tasks) {
    return tasks.sort((a, b) => a.deadline - b.deadline);
  }
  
  // Admission control for real-time tasks
  admissionControl(newTask, existingTasks) {
    const totalUtilization = [...existingTasks, newTask]
      .reduce((sum, task) => sum + (task.executionTime / task.period), 0);
    
    return totalUtilization <= 1.0; // Liu & Layland bound
  }
}
```

### 2. Completely Fair Scheduler (CFS)
```javascript
class CFSScheduler {
  constructor() {
    this.virtualRuntime = new Map();
    this.weights = new Map();
    this.rbtree = new RedBlackTree();
  }
  
  schedule() {
    const nextTask = this.rbtree.minimum();
    if (nextTask) {
      this.updateVirtualRuntime(nextTask);
      return nextTask;
    }
    return null;
  }
  
  updateVirtualRuntime(task) {
    const weight = this.weights.get(task.id) || 1;
    const runtime = this.virtualRuntime.get(task.id) || 0;
    this.virtualRuntime.set(task.id, runtime + (1000 / weight)); // Nice value scaling
  }
}
```

## Performance Optimization Features

### Circuit Breaker Pattern
```javascript
class CircuitBreaker {
  constructor(threshold = 5, timeout = 60000) {
    this.failureThreshold = threshold;
    this.timeout = timeout;
    this.failureCount = 0;
    this.lastFailureTime = null;
    this.state = 'CLOSED'; // CLOSED, OPEN, HALF_OPEN
  }
  
  async execute(operation) {
    if (this.state === 'OPEN') {
      if (Date.now() - this.lastFailureTime > this.timeout) {
        this.state = 'HALF_OPEN';
      } else {
        throw new Error('Circuit breaker is OPEN');
      }
    }
    
    try {
      const result = await operation();
      this.onSuccess();
      return result;
    } catch (error) {
      this.onFailure();
      throw error;
    }
  }
  
  onSuccess() {
    this.failureCount = 0;
    this.state = 'CLOSED';
  }
  
  onFailure() {
    this.failureCount++;
    this.lastFailureTime = Date.now();
    
    if (this.failureCount >= this.failureThreshold) {
      this.state = 'OPEN';
    }
  }
}
```

## Operational Commands

### Load Balancing Commands
```bash
# Initialize load balancer
npx claude-flow agent spawn load-balancer --type coordinator

# Start load balancing
npx claude-flow load-balance --swarm-id <id> --strategy adaptive

# Monitor load distribution
npx claude-flow agent-metrics --type load-balancer

# Adjust balancing parameters
npx claude-flow config-manage --action update --config '{"stealThreshold": 5, "agingBoost": 10}'
```

### Performance Monitoring
```bash
# Real-time load monitoring
npx claude-flow performance-report --format detailed

# Bottleneck analysis
npx claude-flow bottleneck-analyze --component swarm-coordination

# Resource utilization tracking
npx claude-flow metrics-collect --components ["load-balancer", "task-queue"]
```

## Integration Points

### With Other Optimization Agents
- **Performance Monitor**: Provides real-time metrics for load balancing decisions
- **Topology Optimizer**: Coordinates topology changes based on load patterns
- **Resource Allocator**: Optimizes resource distribution across the swarm

### With Swarm Infrastructure
- **Task Orchestrator**: Receives load-balanced task assignments
- **Agent Coordinator**: Provides agent capacity and availability information
- **Memory System**: Stores load balancing history and patterns

## Performance Metrics

### Key Performance Indicators
- **Load Distribution Variance**: Measure of load balance across agents
- **Task Migration Rate**: Frequency of work-stealing operations
- **Queue Latency**: Average time tasks spend in queues
- **Utilization Efficiency**: Percentage of optimal resource utilization
- **Fairness Index**: Measure of fair resource allocation

### Benchmarking
```javascript
// Load balancer benchmarking suite
const benchmarks = {
  async throughputTest(taskCount, agentCount) {
    const startTime = performance.now();
    await this.distributeAndExecute(taskCount, agentCount);
    const endTime = performance.now();
    
    return {
      throughput: taskCount / ((endTime - startTime) / 1000),
      averageLatency: (endTime - startTime) / taskCount
    };
  },
  
  async loadBalanceEfficiency(tasks, agents) {
    const distribution = await this.distributeLoad(tasks, agents);
    const idealLoad = tasks.length / agents.length;
    
    const variance = distribution.reduce((sum, load) => 
      sum + Math.pow(load - idealLoad, 2), 0) / agents.length;
    
    return {
      efficiency: 1 / (1 + variance),
      loadVariance: variance
    };
  }
};
```

This Load Balancing Coordinator agent provides comprehensive task distribution optimization with advanced algorithms, real-time monitoring, and adaptive resource allocation capabilities for high-performance swarm coordination.