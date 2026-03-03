/**
 * MCP Integration Tests
 * Tests MCP tools integration and mock agent interactions
 */

import { describe, test, expect, beforeAll, afterAll, beforeEach, afterEach } from '@jest/globals';
import { testUtils } from '@test/test-helpers';

// Mock MCP client for testing
class MockMCPClient {
  private tools: Map<string, any> = new Map();
  private resources: Map<string, any> = new Map();
  private prompts: Map<string, any> = new Map();

  constructor() {
    this.setupMockTools();
  }

  private setupMockTools() {
    // Mock sublinear-solver tools
    this.tools.set('solve', {
      name: 'solve',
      description: 'Solve a diagonally dominant linear system',
      inputSchema: {
        type: 'object',
        properties: {
          matrix: { type: 'object' },
          vector: { type: 'array' },
          method: { type: 'string', default: 'neumann' }
        }
      }
    });

    this.tools.set('estimateEntry', {
      name: 'estimateEntry',
      description: 'Estimate a single entry of the solution',
      inputSchema: {
        type: 'object',
        properties: {
          matrix: { type: 'object' },
          vector: { type: 'array' },
          row: { type: 'number' },
          column: { type: 'number' }
        }
      }
    });

    // Mock flow-nexus tools
    this.tools.set('swarm_init', {
      name: 'swarm_init',
      description: 'Initialize multi-agent swarm',
      inputSchema: {
        type: 'object',
        properties: {
          topology: { type: 'string', enum: ['hierarchical', 'mesh', 'ring', 'star'] },
          maxAgents: { type: 'number', default: 8 }
        }
      }
    });

    this.tools.set('agent_spawn', {
      name: 'agent_spawn',
      description: 'Create specialized AI agent',
      inputSchema: {
        type: 'object',
        properties: {
          type: { type: 'string', enum: ['researcher', 'coder', 'analyst', 'optimizer'] },
          capabilities: { type: 'array' }
        }
      }
    });

    this.tools.set('neural_train', {
      name: 'neural_train',
      description: 'Train neural network',
      inputSchema: {
        type: 'object',
        properties: {
          config: { type: 'object' },
          tier: { type: 'string', enum: ['nano', 'mini', 'small', 'medium'] }
        }
      }
    });
  }

  async callTool(name: string, parameters: any): Promise<any> {
    const tool = this.tools.get(name);
    if (!tool) {
      throw new Error(`Tool ${name} not found`);
    }

    // Simulate tool execution based on name
    switch (name) {
      case 'solve':
        return this.mockSolve(parameters);
      case 'estimateEntry':
        return this.mockEstimateEntry(parameters);
      case 'swarm_init':
        return this.mockSwarmInit(parameters);
      case 'agent_spawn':
        return this.mockAgentSpawn(parameters);
      case 'neural_train':
        return this.mockNeuralTrain(parameters);
      default:
        return { success: true, result: 'Mock execution completed' };
    }
  }

  private mockSolve(params: any) {
    const { matrix, vector, method = 'neumann' } = params;

    if (!matrix || !vector) {
      throw new Error('Matrix and vector are required');
    }

    // Simulate solving a linear system
    const solution = vector.map(() => Math.random());
    const iterations = Math.floor(Math.random() * 100) + 1;
    const convergence = Math.random() * 0.001;

    return {
      solution,
      iterations,
      convergence,
      method,
      success: true,
      executionTime: Math.random() * 1000
    };
  }

  private mockEstimateEntry(params: any) {
    const { matrix, vector, row, column } = params;

    if (!matrix || !vector || row === undefined || column === undefined) {
      throw new Error('Matrix, vector, row, and column are required');
    }

    return {
      estimate: Math.random(),
      confidence: 0.95,
      samplesUsed: Math.floor(Math.random() * 1000) + 100,
      executionTime: Math.random() * 500
    };
  }

  private mockSwarmInit(params: any) {
    const { topology, maxAgents = 8 } = params;

    return {
      swarmId: `swarm-${Date.now()}`,
      topology,
      maxAgents,
      status: 'initialized',
      nodes: [],
      connections: []
    };
  }

  private mockAgentSpawn(params: any) {
    const { type, capabilities = [] } = params;

    return {
      agentId: `agent-${type}-${Date.now()}`,
      type,
      capabilities,
      status: 'active',
      spawnTime: Date.now()
    };
  }

  private mockNeuralTrain(params: any) {
    const { config, tier = 'nano' } = params;

    return {
      jobId: `job-${Date.now()}`,
      status: 'training',
      tier,
      estimatedDuration: Math.random() * 3600,
      progress: 0,
      modelId: null
    };
  }

  listTools(): Array<{ name: string; description: string }> {
    return Array.from(this.tools.values()).map(tool => ({
      name: tool.name,
      description: tool.description
    }));
  }
}

describe('MCP Integration Tests', () => {
  let mcpClient: MockMCPClient;

  beforeAll(() => {
    mcpClient = new MockMCPClient();
  });

  beforeEach(() => {
    testUtils.mockAgentFactory.clearAgents();
  });

  describe('MCP Tool Integration', () => {
    test('should list available MCP tools', () => {
      const tools = mcpClient.listTools();
      expect(tools.length).toBeGreaterThan(0);
      expect(tools.some(tool => tool.name === 'solve')).toBe(true);
      expect(tools.some(tool => tool.name === 'swarm_init')).toBe(true);
    });

    test('should call sublinear solver tool', async () => {
      const matrix = {
        rows: 3,
        cols: 3,
        format: 'dense',
        data: [
          [4, 1, 0],
          [1, 4, 1],
          [0, 1, 4]
        ]
      };
      const vector = [1, 2, 3];

      const result = await mcpClient.callTool('solve', { matrix, vector });

      expect(result.success).toBe(true);
      expect(result.solution).toHaveLength(3);
      expect(result.iterations).toBeGreaterThan(0);
      expect(result.method).toBe('neumann');
    });

    test('should estimate matrix entry', async () => {
      const matrix = {
        rows: 2,
        cols: 2,
        format: 'dense',
        data: [[2, 1], [1, 2]]
      };
      const vector = [1, 1];

      const result = await mcpClient.callTool('estimateEntry', {
        matrix,
        vector,
        row: 0,
        column: 1
      });

      expect(result.estimate).toBeDefined();
      expect(result.confidence).toBeGreaterThan(0);
      expect(result.samplesUsed).toBeGreaterThan(0);
    });

    test('should initialize swarm', async () => {
      const result = await mcpClient.callTool('swarm_init', {
        topology: 'mesh',
        maxAgents: 5
      });

      expect(result.swarmId).toBeDefined();
      expect(result.topology).toBe('mesh');
      expect(result.maxAgents).toBe(5);
      expect(result.status).toBe('initialized');
    });

    test('should spawn agents', async () => {
      const result = await mcpClient.callTool('agent_spawn', {
        type: 'researcher',
        capabilities: ['search', 'analyze', 'report']
      });

      expect(result.agentId).toBeDefined();
      expect(result.type).toBe('researcher');
      expect(result.capabilities).toEqual(['search', 'analyze', 'report']);
      expect(result.status).toBe('active');
    });

    test('should start neural training', async () => {
      const config = {
        architecture: {
          type: 'feedforward',
          layers: [
            { type: 'dense', units: 128, activation: 'relu' },
            { type: 'dense', units: 64, activation: 'relu' },
            { type: 'dense', units: 1, activation: 'sigmoid' }
          ]
        },
        training: {
          epochs: 100,
          batch_size: 32,
          learning_rate: 0.001
        }
      };

      const result = await mcpClient.callTool('neural_train', {
        config,
        tier: 'small'
      });

      expect(result.jobId).toBeDefined();
      expect(result.status).toBe('training');
      expect(result.tier).toBe('small');
    });

    test('should handle tool errors gracefully', async () => {
      await expect(mcpClient.callTool('nonexistent_tool', {}))
        .rejects.toThrow('Tool nonexistent_tool not found');

      await expect(mcpClient.callTool('solve', {}))
        .rejects.toThrow('Matrix and vector are required');
    });
  });

  describe('Mock Agent Integration', () => {
    test('should create and manage mock agents', () => {
      const agent1 = testUtils.mockAgentFactory.createAgent('agent1', 'researcher', ['search', 'analyze']);
      const agent2 = testUtils.mockAgentFactory.createAgent('agent2', 'coder', ['implement', 'test']);

      expect(agent1.id).toBe('agent1');
      expect(agent1.type).toBe('researcher');
      expect(agent1.capabilities).toEqual(['search', 'analyze']);

      expect(agent2.id).toBe('agent2');
      expect(agent2.type).toBe('coder');
      expect(agent2.capabilities).toEqual(['implement', 'test']);

      const allAgents = testUtils.mockAgentFactory.getAllAgents();
      expect(allAgents).toHaveLength(2);
    });

    test('should send messages between agents', () => {
      const agent1 = testUtils.mockAgentFactory.createAgent('agent1', 'coordinator');
      const agent2 = testUtils.mockAgentFactory.createAgent('agent2', 'worker');

      testUtils.mockAgentFactory.sendMessage('agent2', {
        from: 'agent1',
        type: 'task_assignment',
        payload: { task: 'analyze_data', priority: 'high' }
      });

      const messages = testUtils.mockAgentFactory.processMessages();
      expect(messages).toHaveLength(1);
      expect(messages[0].agentId).toBe('agent2');
      expect(messages[0].message.type).toBe('task_assignment');

      const agent2Updated = testUtils.mockAgentFactory.getAgent('agent2');
      expect(agent2Updated?.messageHistory).toHaveLength(1);
    });

    test('should update agent state', () => {
      const agent = testUtils.mockAgentFactory.createAgent('agent1', 'analyzer');

      testUtils.mockAgentFactory.updateAgentState('agent1', {
        status: 'busy',
        currentTask: 'processing_data',
        progress: 0.5
      });

      const updatedAgent = testUtils.mockAgentFactory.getAgent('agent1');
      expect(updatedAgent?.state.status).toBe('busy');
      expect(updatedAgent?.state.currentTask).toBe('processing_data');
      expect(updatedAgent?.state.progress).toBe(0.5);
    });

    test('should simulate agent collaboration', async () => {
      // Create a team of agents
      const coordinator = testUtils.mockAgentFactory.createAgent('coordinator', 'coordinator', ['orchestrate', 'monitor']);
      const researcher = testUtils.mockAgentFactory.createAgent('researcher', 'researcher', ['search', 'analyze']);
      const coder = testUtils.mockAgentFactory.createAgent('coder', 'coder', ['implement', 'test']);
      const reviewer = testUtils.mockAgentFactory.createAgent('reviewer', 'reviewer', ['review', 'validate']);

      // Simulate a workflow
      // 1. Coordinator assigns research task
      testUtils.mockAgentFactory.sendMessage('researcher', {
        from: 'coordinator',
        type: 'task_assignment',
        payload: { task: 'research_best_practices', deadline: Date.now() + 3600000 }
      });

      // 2. Researcher completes research and sends results
      testUtils.mockAgentFactory.updateAgentState('researcher', { status: 'completed' });
      testUtils.mockAgentFactory.sendMessage('coordinator', {
        from: 'researcher',
        type: 'task_completed',
        payload: {
          task: 'research_best_practices',
          results: ['practice1', 'practice2', 'practice3'],
          confidence: 0.95
        }
      });

      // 3. Coordinator assigns implementation task
      testUtils.mockAgentFactory.sendMessage('coder', {
        from: 'coordinator',
        type: 'task_assignment',
        payload: {
          task: 'implement_practices',
          requirements: ['practice1', 'practice2'],
          priority: 'high'
        }
      });

      // 4. Coder implements and sends for review
      testUtils.mockAgentFactory.updateAgentState('coder', { status: 'completed' });
      testUtils.mockAgentFactory.sendMessage('reviewer', {
        from: 'coder',
        type: 'review_request',
        payload: {
          implementation: 'code_implementation',
          tests: ['test1', 'test2'],
          coverage: 0.95
        }
      });

      // Process all messages
      const messages = testUtils.mockAgentFactory.processMessages();
      expect(messages).toHaveLength(4);

      // Verify message flow
      const researcherMessages = testUtils.mockAgentFactory.getAgent('researcher')?.messageHistory;
      const coderMessages = testUtils.mockAgentFactory.getAgent('coder')?.messageHistory;
      const reviewerMessages = testUtils.mockAgentFactory.getAgent('reviewer')?.messageHistory;
      const coordinatorMessages = testUtils.mockAgentFactory.getAgent('coordinator')?.messageHistory;

      expect(researcherMessages).toHaveLength(1);
      expect(coderMessages).toHaveLength(1);
      expect(reviewerMessages).toHaveLength(1);
      expect(coordinatorMessages).toHaveLength(1);
    });
  });

  describe('Psycho-Symbolic Reasoner MCP Integration', () => {
    test('should integrate with graph reasoner through MCP', async () => {
      // Simulate using MCP to call WASM graph reasoner
      const mockWasmCall = async (operation: string, data: any) => {
        switch (operation) {
          case 'add_fact':
            return `fact-${Date.now()}`;
          case 'query':
            return {
              facts: [
                { subject: data.subject, predicate: 'knows', object: 'Charlie' },
                { subject: data.subject, predicate: 'loves', object: 'Alice' }
              ],
              execution_time_ms: 12.5
            };
          case 'infer':
            return [
              { fact: { subject: 'Socrates', predicate: 'is', object: 'mortal' }, confidence: 1.0 }
            ];
          default:
            throw new Error(`Unknown operation: ${operation}`);
        }
      };

      // Create agent that uses graph reasoner
      const reasoningAgent = testUtils.mockAgentFactory.createAgent('reasoning_agent', 'reasoner', ['graph_analysis', 'inference']);

      // Agent adds facts
      const factId = await mockWasmCall('add_fact', {
        subject: 'Bob',
        predicate: 'knows',
        object: 'Alice'
      });

      testUtils.mockAgentFactory.updateAgentState('reasoning_agent', {
        facts_added: [factId],
        operation_count: 1
      });

      // Agent performs query
      const queryResult = await mockWasmCall('query', {
        subject: 'Bob',
        predicate: null,
        object: null
      });

      testUtils.mockAgentFactory.updateAgentState('reasoning_agent', {
        last_query_result: queryResult,
        operation_count: 2
      });

      // Agent performs inference
      const inferenceResult = await mockWasmCall('infer', { max_iterations: 5 });

      testUtils.mockAgentFactory.updateAgentState('reasoning_agent', {
        last_inference_result: inferenceResult,
        operation_count: 3
      });

      const agent = testUtils.mockAgentFactory.getAgent('reasoning_agent');
      expect(agent?.state.operation_count).toBe(3);
      expect(agent?.state.last_query_result.facts).toHaveLength(2);
      expect(agent?.state.last_inference_result).toHaveLength(1);
    });

    test('should integrate with planner through MCP', async () => {
      const mockPlannerCall = async (operation: string, data: any) => {
        switch (operation) {
          case 'set_state':
            return true;
          case 'add_action':
            return true;
          case 'add_goal':
            return true;
          case 'plan':
            return {
              success: true,
              plan: {
                id: 'plan-1',
                steps: [
                  { action_id: 'move_to_store', step_number: 1 },
                  { action_id: 'buy_item', step_number: 2 }
                ],
                total_cost: 15.0
              }
            };
          default:
            throw new Error(`Unknown operation: ${operation}`);
        }
      };

      const planningAgent = testUtils.mockAgentFactory.createAgent('planning_agent', 'planner', ['goal_planning', 'action_sequencing']);

      // Set initial state
      await mockPlannerCall('set_state', { key: 'location', value: 'home' });
      await mockPlannerCall('set_state', { key: 'has_money', value: true });

      // Add actions
      await mockPlannerCall('add_action', {
        id: 'move_to_store',
        preconditions: [{ state_key: 'has_money', required_value: true }],
        effects: [{ state_key: 'location', value: 'store' }],
        cost: 5.0
      });

      await mockPlannerCall('add_action', {
        id: 'buy_item',
        preconditions: [{ state_key: 'location', required_value: 'store' }],
        effects: [{ state_key: 'has_item', value: true }],
        cost: 10.0
      });

      // Add goal
      await mockPlannerCall('add_goal', {
        id: 'acquire_item',
        conditions: [{ state_key: 'has_item', target_value: true }]
      });

      // Create plan
      const planResult = await mockPlannerCall('plan', { goal_id: 'acquire_item' });

      testUtils.mockAgentFactory.updateAgentState('planning_agent', {
        current_plan: planResult.plan,
        planning_successful: planResult.success
      });

      const agent = testUtils.mockAgentFactory.getAgent('planning_agent');
      expect(agent?.state.planning_successful).toBe(true);
      expect(agent?.state.current_plan.steps).toHaveLength(2);
      expect(agent?.state.current_plan.total_cost).toBe(15.0);
    });

    test('should integrate with text extractors through MCP', async () => {
      const mockExtractorCall = async (operation: string, data: any) => {
        switch (operation) {
          case 'analyze_sentiment':
            return {
              overall_score: data.text.includes('love') ? 0.8 : (data.text.includes('hate') ? -0.8 : 0.0),
              dominant_sentiment: data.text.includes('love') ? 'positive' : (data.text.includes('hate') ? 'negative' : 'neutral'),
              confidence: 0.9
            };
          case 'extract_preferences':
            return data.text.includes('prefer') ? [
              { item: 'coffee', category: 'beverages', strength: 0.7 }
            ] : [];
          case 'detect_emotions':
            return data.text.includes('excited') ? [
              { emotion_type: 'Joy', intensity: 0.8, confidence: 0.9 }
            ] : [];
          default:
            throw new Error(`Unknown operation: ${operation}`);
        }
      };

      const extractorAgent = testUtils.mockAgentFactory.createAgent('extractor_agent', 'extractor', ['sentiment_analysis', 'preference_extraction']);

      const testText = "I love this product! I prefer it over all competitors. I'm so excited to use it!";

      // Analyze sentiment
      const sentimentResult = await mockExtractorCall('analyze_sentiment', { text: testText });

      // Extract preferences
      const preferenceResult = await mockExtractorCall('extract_preferences', { text: testText });

      // Detect emotions
      const emotionResult = await mockExtractorCall('detect_emotions', { text: testText });

      testUtils.mockAgentFactory.updateAgentState('extractor_agent', {
        last_sentiment: sentimentResult,
        last_preferences: preferenceResult,
        last_emotions: emotionResult,
        analysis_count: 3
      });

      const agent = testUtils.mockAgentFactory.getAgent('extractor_agent');
      expect(agent?.state.last_sentiment.dominant_sentiment).toBe('positive');
      expect(agent?.state.last_preferences).toHaveLength(1);
      expect(agent?.state.last_emotions).toHaveLength(1);
      expect(agent?.state.analysis_count).toBe(3);
    });
  });

  describe('Multi-Agent Coordination with MCP', () => {
    test('should coordinate agents using MCP tools', async () => {
      // Initialize swarm
      const swarmResult = await mcpClient.callTool('swarm_init', {
        topology: 'hierarchical',
        maxAgents: 4
      });

      // Spawn coordinated agents
      const coordinator = await mcpClient.callTool('agent_spawn', {
        type: 'coordinator',
        capabilities: ['orchestrate', 'monitor', 'coordinate']
      });

      const researcher = await mcpClient.callTool('agent_spawn', {
        type: 'researcher',
        capabilities: ['search', 'analyze', 'synthesize']
      });

      const reasoner = await mcpClient.callTool('agent_spawn', {
        type: 'analyst',
        capabilities: ['logical_reasoning', 'inference', 'validation']
      });

      const planner = await mcpClient.callTool('agent_spawn', {
        type: 'optimizer',
        capabilities: ['planning', 'optimization', 'execution']
      });

      // Create mock agents for local simulation
      testUtils.mockAgentFactory.createAgent(coordinator.agentId, 'coordinator', coordinator.capabilities);
      testUtils.mockAgentFactory.createAgent(researcher.agentId, 'researcher', researcher.capabilities);
      testUtils.mockAgentFactory.createAgent(reasoner.agentId, 'analyst', reasoner.capabilities);
      testUtils.mockAgentFactory.createAgent(planner.agentId, 'optimizer', planner.capabilities);

      // Simulate complex multi-agent task
      const task = {
        id: 'complex_analysis',
        description: 'Analyze user feedback and create improvement plan',
        input: "Users love the interface but hate the slow performance. They prefer mobile apps over web versions.",
        phases: ['research', 'reasoning', 'planning']
      };

      // Phase 1: Research (Text Analysis)
      testUtils.mockAgentFactory.sendMessage(researcher.agentId, {
        from: coordinator.agentId,
        type: 'task_assignment',
        payload: {
          phase: 'research',
          action: 'analyze_feedback',
          data: task.input
        }
      });

      // Simulate research completion
      testUtils.mockAgentFactory.updateAgentState(researcher.agentId, {
        phase: 'research',
        status: 'completed',
        results: {
          sentiment: { overall_score: 0.2, mixed_aspects: true },
          preferences: [
            { item: 'mobile apps', strength: 0.8 },
            { item: 'fast performance', strength: 0.9 }
          ],
          issues: ['slow performance']
        }
      });

      // Phase 2: Reasoning (Knowledge Graph)
      testUtils.mockAgentFactory.sendMessage(reasoner.agentId, {
        from: coordinator.agentId,
        type: 'task_assignment',
        payload: {
          phase: 'reasoning',
          action: 'build_knowledge_graph',
          data: testUtils.mockAgentFactory.getAgent(researcher.agentId)?.state.results
        }
      });

      // Simulate reasoning completion
      testUtils.mockAgentFactory.updateAgentState(reasoner.agentId, {
        phase: 'reasoning',
        status: 'completed',
        results: {
          facts: [
            { subject: 'users', predicate: 'prefer', object: 'mobile_apps' },
            { subject: 'users', predicate: 'dislike', object: 'slow_performance' },
            { subject: 'performance_issues', predicate: 'causes', object: 'user_dissatisfaction' }
          ],
          inferences: [
            { conclusion: 'mobile_optimization_needed', confidence: 0.9 },
            { conclusion: 'performance_optimization_critical', confidence: 0.95 }
          ]
        }
      });

      // Phase 3: Planning (Goal-Oriented Action Planning)
      testUtils.mockAgentFactory.sendMessage(planner.agentId, {
        from: coordinator.agentId,
        type: 'task_assignment',
        payload: {
          phase: 'planning',
          action: 'create_improvement_plan',
          data: testUtils.mockAgentFactory.getAgent(reasoner.agentId)?.state.results
        }
      });

      // Simulate planning completion
      testUtils.mockAgentFactory.updateAgentState(planner.agentId, {
        phase: 'planning',
        status: 'completed',
        results: {
          plan: {
            goals: [
              { id: 'improve_performance', priority: 'high' },
              { id: 'enhance_mobile_experience', priority: 'medium' }
            ],
            actions: [
              { id: 'optimize_backend', cost: 20, duration: 'weeks' },
              { id: 'improve_mobile_ui', cost: 15, duration: 'weeks' },
              { id: 'implement_caching', cost: 5, duration: 'days' }
            ],
            estimated_total_cost: 40,
            success_probability: 0.85
          }
        }
      });

      // Coordinator aggregates results
      const allAgents = testUtils.mockAgentFactory.getAllAgents();
      const completedPhases = allAgents.filter(agent =>
        agent.state.status === 'completed'
      );

      expect(completedPhases).toHaveLength(3); // researcher, reasoner, planner
      expect(swarmResult.swarmId).toBeDefined();
      expect(coordinator.agentId).toBeDefined();

      // Verify coordination results
      const researchResults = testUtils.mockAgentFactory.getAgent(researcher.agentId)?.state.results;
      const reasoningResults = testUtils.mockAgentFactory.getAgent(reasoner.agentId)?.state.results;
      const planningResults = testUtils.mockAgentFactory.getAgent(planner.agentId)?.state.results;

      expect(researchResults?.sentiment).toBeDefined();
      expect(reasoningResults?.inferences).toHaveLength(2);
      expect(planningResults?.plan.actions).toHaveLength(3);
    });

    test('should handle agent failures and recovery', async () => {
      const coordinator = testUtils.mockAgentFactory.createAgent('coordinator', 'coordinator');
      const worker1 = testUtils.mockAgentFactory.createAgent('worker1', 'worker');
      const worker2 = testUtils.mockAgentFactory.createAgent('worker2', 'worker');

      // Assign task to worker1
      testUtils.mockAgentFactory.sendMessage('worker1', {
        from: 'coordinator',
        type: 'task_assignment',
        payload: { task: 'process_data', timeout: 5000 }
      });

      // Simulate worker1 failure
      testUtils.mockAgentFactory.updateAgentState('worker1', {
        status: 'failed',
        error: 'timeout_exceeded',
        task_progress: 0.3
      });

      // Coordinator detects failure and reassigns to worker2
      testUtils.mockAgentFactory.sendMessage('worker2', {
        from: 'coordinator',
        type: 'task_assignment',
        payload: {
          task: 'process_data',
          previous_worker: 'worker1',
          recovery_mode: true,
          partial_progress: 0.3
        }
      });

      // Worker2 completes the task
      testUtils.mockAgentFactory.updateAgentState('worker2', {
        status: 'completed',
        task_result: 'data_processed',
        recovery_successful: true
      });

      const worker1State = testUtils.mockAgentFactory.getAgent('worker1')?.state;
      const worker2State = testUtils.mockAgentFactory.getAgent('worker2')?.state;

      expect(worker1State?.status).toBe('failed');
      expect(worker2State?.status).toBe('completed');
      expect(worker2State?.recovery_successful).toBe(true);
    });
  });

  describe('Performance and Stress Testing', () => {
    test('should handle high-frequency MCP calls', async () => {
      const collector = testUtils.performanceCollector;
      collector.start();

      const promises = [];
      for (let i = 0; i < 100; i++) {
        promises.push(mcpClient.callTool('estimateEntry', {
          matrix: { rows: 2, cols: 2, format: 'dense', data: [[2, 1], [1, 2]] },
          vector: [1, 1],
          row: 0,
          column: 0
        }));
        collector.incrementOperation();
      }

      const results = await Promise.all(promises);
      const metrics = collector.stop();

      expect(results).toHaveLength(100);
      expect(metrics.executionTime).toBeLessThan(5000); // Under 5 seconds
      expect(results.every(r => r.estimate !== undefined)).toBe(true);
    });

    test('should handle large agent networks', () => {
      const agentCount = 1000;
      const agents = [];

      // Create large number of agents
      for (let i = 0; i < agentCount; i++) {
        const agent = testUtils.mockAgentFactory.createAgent(
          `agent_${i}`,
          i % 4 === 0 ? 'coordinator' : 'worker',
          [`capability_${i % 5}`]
        );
        agents.push(agent);
      }

      // Send messages between agents
      for (let i = 0; i < agentCount / 10; i++) {
        const sourceId = `agent_${i}`;
        const targetId = `agent_${(i + 1) % agentCount}`;

        testUtils.mockAgentFactory.sendMessage(targetId, {
          from: sourceId,
          type: 'ping',
          payload: { timestamp: Date.now() }
        });
      }

      const allAgents = testUtils.mockAgentFactory.getAllAgents();
      const messages = testUtils.mockAgentFactory.processMessages();

      expect(allAgents).toHaveLength(agentCount);
      expect(messages).toHaveLength(agentCount / 10);
    });
  });
});