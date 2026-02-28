import { create } from 'zustand';
import type { MCPTool, MCPResult } from '../types';

interface MCPState {
  tools: MCPTool[];
  results: MCPResult[];
  isConnected: boolean;
  activeTools: string[];

  // Actions
  setTools: (tools: MCPTool[]) => void;
  updateTool: (toolId: string, updates: Partial<MCPTool>) => void;
  addResult: (result: MCPResult) => void;
  clearResults: () => void;
  setConnected: (connected: boolean) => void;
  executeTool: (toolId: string, params?: Record<string, unknown>) => Promise<MCPResult>;
}

const defaultTools: MCPTool[] = [
  // Swarm Tools
  {
    id: 'swarm_init',
    name: 'Swarm Initialize',
    description: 'Initialize a new swarm with specified topology',
    category: 'swarm',
    status: 'ready',
  },
  {
    id: 'swarm_status',
    name: 'Swarm Status',
    description: 'Get current swarm status and agent information',
    category: 'swarm',
    status: 'ready',
  },
  {
    id: 'swarm_monitor',
    name: 'Swarm Monitor',
    description: 'Monitor swarm activity in real-time',
    category: 'swarm',
    status: 'ready',
  },
  // Agent Tools
  {
    id: 'agent_spawn',
    name: 'Agent Spawn',
    description: 'Spawn a new agent in the swarm',
    category: 'agent',
    status: 'ready',
  },
  {
    id: 'agent_list',
    name: 'Agent List',
    description: 'List all active agents in the swarm',
    category: 'agent',
    status: 'ready',
  },
  {
    id: 'agent_metrics',
    name: 'Agent Metrics',
    description: 'Get performance metrics for agents',
    category: 'agent',
    status: 'ready',
  },
  // Task Tools
  {
    id: 'task_orchestrate',
    name: 'Task Orchestrate',
    description: 'Orchestrate a task across the swarm',
    category: 'task',
    status: 'ready',
  },
  {
    id: 'task_status',
    name: 'Task Status',
    description: 'Check progress of running tasks',
    category: 'task',
    status: 'ready',
  },
  {
    id: 'task_results',
    name: 'Task Results',
    description: 'Retrieve results from completed tasks',
    category: 'task',
    status: 'ready',
  },
  // Memory Tools
  {
    id: 'memory_usage',
    name: 'Memory Usage',
    description: 'Get current memory usage statistics',
    category: 'memory',
    status: 'ready',
  },
  // Neural Tools
  {
    id: 'neural_status',
    name: 'Neural Status',
    description: 'Get neural agent status and performance metrics',
    category: 'neural',
    status: 'ready',
  },
  {
    id: 'neural_train',
    name: 'Neural Train',
    description: 'Train neural agents with sample tasks',
    category: 'neural',
    status: 'ready',
  },
  {
    id: 'neural_patterns',
    name: 'Neural Patterns',
    description: 'Get cognitive pattern information',
    category: 'neural',
    status: 'ready',
  },
  // GitHub Tools
  {
    id: 'github_repo_analyze',
    name: 'Repository Analyze',
    description: 'Analyze GitHub repository structure and code',
    category: 'github',
    status: 'ready',
  },
  {
    id: 'github_pr_manage',
    name: 'PR Management',
    description: 'Manage pull requests and reviews',
    category: 'github',
    status: 'ready',
  },
];

export const useMCPStore = create<MCPState>((set, get) => ({
  tools: defaultTools,
  results: [],
  isConnected: true,
  activeTools: [],

  setTools: (tools) => set({ tools }),

  updateTool: (toolId, updates) =>
    set((state) => ({
      tools: state.tools.map((t) =>
        t.id === toolId ? { ...t, ...updates } : t
      ),
    })),

  addResult: (result) =>
    set((state) => ({
      results: [result, ...state.results].slice(0, 50), // Keep last 50 results
    })),

  clearResults: () => set({ results: [] }),
  setConnected: (connected) => set({ isConnected: connected }),

  executeTool: async (toolId, params) => {
    const { updateTool, addResult } = get();
    const startTime = performance.now();

    updateTool(toolId, { status: 'running' });
    set((state) => ({ activeTools: [...state.activeTools, toolId] }));

    try {
      // Simulate tool execution
      await new Promise((resolve) =>
        setTimeout(resolve, 500 + Math.random() * 1500)
      );

      const result: MCPResult = {
        toolId,
        success: true,
        data: {
          message: `Tool ${toolId} executed successfully`,
          params,
          timestamp: new Date().toISOString(),
        },
        timestamp: new Date(),
        duration: performance.now() - startTime,
      };

      updateTool(toolId, { status: 'ready', lastRun: new Date() });
      addResult(result);

      set((state) => ({
        activeTools: state.activeTools.filter((id) => id !== toolId),
      }));

      console.log(`[MCP] Tool ${toolId} completed:`, result);
      return result;
    } catch (error) {
      const result: MCPResult = {
        toolId,
        success: false,
        error: error instanceof Error ? error.message : 'Unknown error',
        timestamp: new Date(),
        duration: performance.now() - startTime,
      };

      updateTool(toolId, { status: 'error' });
      addResult(result);

      set((state) => ({
        activeTools: state.activeTools.filter((id) => id !== toolId),
      }));

      return result;
    }
  },
}));
