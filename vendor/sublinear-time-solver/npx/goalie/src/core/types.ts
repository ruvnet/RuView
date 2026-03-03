/**
 * Core GOAP Types and Interfaces
 * Following STRIPS-style planning with preconditions and effects
 */

export interface WorldState {
  [key: string]: any;
}

export interface Precondition {
  key: string;
  value: any;
  operator?: 'equals' | 'exists' | 'not_exists' | 'greater' | 'less' | 'contains';
}

export interface Effect {
  key: string;
  value: any;
  operation?: 'set' | 'add' | 'remove' | 'increment' | 'decrement';
}

export interface GoapAction {
  name: string;
  cost: number;
  preconditions: Precondition[];
  effects: Effect[];
  execute: (state: WorldState, params?: any) => Promise<{
    success: boolean;
    newState: WorldState;
    data?: any;
    error?: string;
  }>;
  validate?: (state: WorldState) => boolean;
  rollback?: (state: WorldState) => Promise<WorldState>;
}

export interface GoapGoal {
  name: string;
  conditions: Precondition[];
  priority: number;
  timeout?: number;
}

export interface PlanStep {
  action: GoapAction;
  params?: any;
  estimatedCost: number;
  expectedState: WorldState;
}

export interface GoapPlan {
  id: string;
  goal: GoapGoal;
  steps: PlanStep[];
  totalCost: number;
  created: Date;
  status: 'pending' | 'executing' | 'completed' | 'failed' | 'replanning';
}

export interface PlanningContext {
  currentState: WorldState;
  goal: GoapGoal;
  availableActions: GoapAction[];
  maxDepth?: number;
  maxCost?: number;
  heuristic?: (state: WorldState, goal: GoapGoal) => number;
}

export interface SearchNode {
  state: WorldState;
  action?: GoapAction;
  parent?: SearchNode;
  gCost: number; // Actual cost from start
  hCost: number; // Heuristic cost to goal
  fCost: number; // Total cost (g + h)
  depth: number;
}

export interface PlanExecutionResult {
  success: boolean;
  finalState: WorldState;
  executedSteps: number;
  failedAt?: number;
  error?: string;
  data?: any;
  replanned?: boolean;
  planHistory: GoapPlan[];
}

// Plugin system types
export interface PluginHooks {
  onPlanStart?: (context: PlanningContext) => Promise<void> | void;
  beforeSearch?: (context: PlanningContext) => Promise<void> | void;
  afterSearch?: (plan: GoapPlan | null, context: PlanningContext) => Promise<void> | void;
  beforeExecute?: (step: PlanStep, state: WorldState) => Promise<void> | void;
  afterExecute?: (step: PlanStep, result: any, state: WorldState) => Promise<void> | void;
  onReplan?: (failedStep: PlanStep, state: WorldState) => Promise<void> | void;
  onPlanComplete?: (result: PlanExecutionResult) => Promise<void> | void;
  onError?: (error: Error, context: any) => Promise<void> | void;
}

export interface GoapPlugin {
  name: string;
  version: string;
  description?: string;
  hooks: PluginHooks;
  initialize?: () => Promise<void> | void;
  cleanup?: () => Promise<void> | void;
  execute?: (params: any) => Promise<any>;
}

// Advanced Reasoning Engine integration types
export interface AdvancedReasoning {
  analyze: (state: WorldState, goal: GoapGoal) => Promise<{
    insights: string[];
    suggestedActions: string[];
    confidence: number;
  }>;
  enhance: (plan: GoapPlan) => Promise<GoapPlan>;
  predict: (action: GoapAction, state: WorldState) => Promise<{
    likelihood: number;
    alternatives: GoapAction[];
  }>;
}

// MCP tool interfaces
export interface SearchRequest {
  query: string;
  domains?: string[];
  recency?: 'hour' | 'day' | 'week' | 'month' | 'year';
  mode?: 'web' | 'academic';
  maxResults?: number;
  model?: string;
  enableReasoning?: boolean;
  planningTimeout?: number;
  // Pagination and output options
  pagination?: {
    page?: number;
    pageSize?: number;
  };
  outputToFile?: boolean;
  outputFormat?: 'json' | 'markdown' | 'both';
  outputPath?: string;
  useQuerySubfolder?: boolean;
  // Ed25519 anti-hallucination options
  ed25519Verification?: {
    enabled: boolean;
    requireSignatures?: boolean;
    signResult?: boolean;
    privateKey?: string;
    keyId?: string;
    certId?: string;
    trustedIssuers?: string[];
  };
}

export interface SearchResult {
  answer: string;
  citations: Array<{
    title: string;
    url: string;
    snippet: string;
    publishDate?: string;
  }>;
  planLog: string[];
  usage: {
    tokens: number;
    cost: number;
  };
  reasoning?: {
    insights: string[];
    confidence: number;
  };
  paginationInfo?: {
    currentPage: number;
    totalPages: number;
    totalResults: number;
    pageSize: number;
  };
  metadata: {
    planId: string;
    executionTime: number;
    replanned: boolean;
    savedFiles?: string[];
    ed25519Verification?: any;
    ed25519Signature?: string;
    ed25519KeyId?: string;
  };
}