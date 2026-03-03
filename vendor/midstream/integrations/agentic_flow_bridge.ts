/**
 * Bridge between Lean Agentic Learning System and agentic-flow
 *
 * This module integrates the Lean Agentic system with the agentic-flow
 * orchestration platform for multi-agent coordination.
 */

import { LeanAgenticClient, Context, ProcessingResult } from '@midstream/lean-agentic';

// agentic-flow types (based on npm package)
interface AgenticFlowConfig {
  agents?: Agent[];
  workflows?: Workflow[];
  reasoningBank?: ReasoningBank;
}

interface Agent {
  id: string;
  name: string;
  type: string;
  capabilities: string[];
  config: Record<string, any>;
}

interface Workflow {
  id: string;
  name: string;
  steps: WorkflowStep[];
}

interface WorkflowStep {
  id: string;
  agentId: string;
  action: string;
  inputs: Record<string, any>;
}

interface ReasoningBank {
  memories: Memory[];
  learnings: Learning[];
}

interface Memory {
  id: string;
  content: string;
  timestamp: number;
  confidence: number;
}

interface Learning {
  id: string;
  pattern: string;
  outcome: string;
  reward: number;
}

/**
 * Bridge class for integrating Lean Agentic with agentic-flow
 */
export class AgenticFlowBridge {
  private leanClient: LeanAgenticClient;
  private flowConfig: AgenticFlowConfig;
  private reasoningBank: ReasoningBank;

  constructor(
    baseURL: string,
    flowConfig: AgenticFlowConfig = {}
  ) {
    this.leanClient = new LeanAgenticClient(baseURL, {
      enableFormalVerification: true,
      learningRate: 0.01,
      maxPlanningDepth: 5,
      actionThreshold: 0.7,
      enableMultiAgent: true,
      kgUpdateFreq: 100,
    });

    this.flowConfig = flowConfig;
    this.reasoningBank = flowConfig.reasoningBank || {
      memories: [],
      learnings: [],
    };
  }

  /**
   * Execute a workflow using Lean Agentic processing
   */
  async executeWorkflow(
    workflowId: string,
    inputs: Record<string, any>,
    context: Context
  ): Promise<WorkflowResult> {
    const workflow = this.flowConfig.workflows?.find(w => w.id === workflowId);

    if (!workflow) {
      throw new Error(`Workflow ${workflowId} not found`);
    }

    const results: StepResult[] = [];

    for (const step of workflow.steps) {
      const agent = this.flowConfig.agents?.find(a => a.id === step.agentId);

      if (!agent) {
        throw new Error(`Agent ${step.agentId} not found`);
      }

      // Process step through Lean Agentic system
      const stepInput = this.buildStepInput(step, inputs, results);
      const result = await this.leanClient.processChunk(stepInput, context);

      // Store in reasoning bank
      this.addMemory({
        id: `${workflowId}_${step.id}`,
        content: stepInput,
        timestamp: Date.now(),
        confidence: result.observation.success ? 0.9 : 0.5,
      });

      this.addLearning({
        id: `${workflowId}_${step.id}_learning`,
        pattern: step.action,
        outcome: result.observation.result,
        reward: result.reward,
      });

      results.push({
        stepId: step.id,
        agentId: step.agentId,
        result: result,
        success: result.observation.success,
      });

      // Update context for next step
      context.history.push(stepInput);
      context.history.push(result.observation.result);
    }

    return {
      workflowId,
      results,
      success: results.every(r => r.success),
      totalReward: results.reduce((sum, r) => sum + r.result.reward, 0),
    };
  }

  /**
   * Create a multi-agent swarm using Lean Agentic coordination
   */
  async createSwarm(
    agentIds: string[],
    task: string,
    context: Context
  ): Promise<SwarmResult> {
    const agents = agentIds.map(id =>
      this.flowConfig.agents?.find(a => a.id === id)
    ).filter(Boolean) as Agent[];

    if (agents.length === 0) {
      throw new Error('No valid agents found');
    }

    // Each agent processes the task independently
    const agentResults = await Promise.all(
      agents.map(async (agent) => {
        const agentContext = { ...context, sessionId: `${context.sessionId}_${agent.id}` };
        const result = await this.leanClient.processChunk(task, agentContext);

        return {
          agentId: agent.id,
          agentName: agent.name,
          result,
        };
      })
    );

    // Aggregate results using voting or averaging
    const consensusResult = this.buildConsensus(agentResults);

    // Store swarm learning
    this.addLearning({
      id: `swarm_${Date.now()}`,
      pattern: `swarm_task: ${task}`,
      outcome: consensusResult.action.description,
      reward: consensusResult.averageReward,
    });

    return {
      task,
      agentResults,
      consensus: consensusResult,
      timestamp: Date.now(),
    };
  }

  /**
   * Query the reasoning bank for patterns
   */
  queryReasoningBank(pattern: string, limit: number = 10): Learning[] {
    return this.reasoningBank.learnings
      .filter(l => l.pattern.includes(pattern))
      .sort((a, b) => b.reward - a.reward)
      .slice(0, limit);
  }

  /**
   * Get memories from reasoning bank
   */
  getMemories(since?: number): Memory[] {
    if (since) {
      return this.reasoningBank.memories.filter(m => m.timestamp >= since);
    }
    return this.reasoningBank.memories;
  }

  /**
   * Export reasoning bank for persistence
   */
  exportReasoningBank(): ReasoningBank {
    return {
      memories: [...this.reasoningBank.memories],
      learnings: [...this.reasoningBank.learnings],
    };
  }

  /**
   * Import reasoning bank from previous session
   */
  importReasoningBank(bank: ReasoningBank): void {
    this.reasoningBank = {
      memories: [...bank.memories],
      learnings: [...bank.learnings],
    };
  }

  private buildStepInput(
    step: WorkflowStep,
    workflowInputs: Record<string, any>,
    previousResults: StepResult[]
  ): string {
    // Combine workflow inputs and previous results
    const context = {
      ...workflowInputs,
      previousResults: previousResults.map(r => r.result.observation.result),
    };

    return `Execute ${step.action} with inputs: ${JSON.stringify(context)}`;
  }

  private buildConsensus(results: AgentResult[]): ConsensusResult {
    const averageReward = results.reduce((sum, r) => sum + r.result.reward, 0) / results.length;

    // Simple majority voting for action type
    const actionCounts = new Map<string, number>();
    results.forEach(r => {
      const action = r.result.action.actionType;
      actionCounts.set(action, (actionCounts.get(action) || 0) + 1);
    });

    const consensusAction = Array.from(actionCounts.entries())
      .sort((a, b) => b[1] - a[1])[0][0];

    return {
      action: results.find(r => r.result.action.actionType === consensusAction)!.result.action,
      averageReward,
      agreement: actionCounts.get(consensusAction)! / results.length,
      participatingAgents: results.length,
    };
  }

  private addMemory(memory: Memory): void {
    this.reasoningBank.memories.push(memory);

    // Keep only last 10000 memories
    if (this.reasoningBank.memories.length > 10000) {
      this.reasoningBank.memories = this.reasoningBank.memories.slice(-10000);
    }
  }

  private addLearning(learning: Learning): void {
    this.reasoningBank.learnings.push(learning);

    // Keep only last 5000 learnings
    if (this.reasoningBank.learnings.length > 5000) {
      this.reasoningBank.learnings = this.reasoningBank.learnings.slice(-5000);
    }
  }
}

// Result types
interface StepResult {
  stepId: string;
  agentId: string;
  result: ProcessingResult;
  success: boolean;
}

interface WorkflowResult {
  workflowId: string;
  results: StepResult[];
  success: boolean;
  totalReward: number;
}

interface AgentResult {
  agentId: string;
  agentName: string;
  result: ProcessingResult;
}

interface ConsensusResult {
  action: any;
  averageReward: number;
  agreement: number;
  participatingAgents: number;
}

interface SwarmResult {
  task: string;
  agentResults: AgentResult[];
  consensus: ConsensusResult;
  timestamp: number;
}

// Example usage
export async function example() {
  const bridge = new AgenticFlowBridge('http://localhost:8080', {
    agents: [
      {
        id: 'agent_1',
        name: 'Weather Agent',
        type: 'specialist',
        capabilities: ['weather', 'forecasting'],
        config: {},
      },
      {
        id: 'agent_2',
        name: 'Calendar Agent',
        type: 'specialist',
        capabilities: ['calendar', 'scheduling'],
        config: {},
      },
    ],
    workflows: [
      {
        id: 'weather_workflow',
        name: 'Weather Check Workflow',
        steps: [
          {
            id: 'step_1',
            agentId: 'agent_1',
            action: 'check_weather',
            inputs: { location: 'Tokyo' },
          },
        ],
      },
    ],
  });

  // Execute workflow
  const context = bridge['leanClient'].createContext('session_001');
  const result = await bridge.executeWorkflow('weather_workflow', { location: 'Tokyo' }, context);

  console.log('Workflow result:', result);

  // Create swarm
  const swarmResult = await bridge.createSwarm(
    ['agent_1', 'agent_2'],
    'What should I do today?',
    context
  );

  console.log('Swarm result:', swarmResult);

  // Query reasoning bank
  const learnings = bridge.queryReasoningBank('weather');
  console.log('Learnings:', learnings);
}
