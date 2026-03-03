// Base types for WASM module interfaces
export interface WasmModule {
  memory: WebAssembly.Memory;
  __wbindgen_malloc(size: number): number;
  __wbindgen_free(ptr: number, size: number): void;
  __wbindgen_realloc(ptr: number, oldSize: number, newSize: number): number;
}

// Graph Reasoner Types
export interface GraphReasonerWasm extends WasmModule {
  GraphReasoner: {
    new(): GraphReasonerInstance;
  };
}

export interface GraphReasonerInstance {
  add_fact(subject: string, predicate: string, object: string): string;
  add_rule(rule_json: string): boolean;
  query(query_json: string): string;
  infer(max_iterations?: number): string;
  get_graph_stats(): string;
  free(): void;
}

// Text Extractor Types
export interface TextExtractorWasm extends WasmModule {
  TextExtractor: {
    new(): TextExtractorInstance;
  };
}

export interface TextExtractorInstance {
  analyze_sentiment(text: string): string;
  extract_preferences(text: string): string;
  detect_emotions(text: string): string;
  analyze_all(text: string): string;
  free(): void;
}

// Planner System Types
export interface PlannerSystemWasm extends WasmModule {
  PlannerSystem: {
    new(): PlannerSystemInstance;
  };
}

export interface PlannerSystemInstance {
  set_state(key: string, value: string): boolean;
  get_state(key: string): string;
  add_action(action_json: string): boolean;
  add_goal(goal_json: string): boolean;
  plan(goal_id: string): string;
  plan_to_state(target_state_json: string): string;
  execute_plan(plan_json: string): string;
  add_rule(rule_json: string): boolean;
  evaluate_rules(): string;
  get_world_state(): string;
  get_available_actions(): string;
  free(): void;
}

// Domain-specific types
export interface Entity {
  id: string;
  name: string;
  attributes: Record<string, any>;
}

export interface Fact {
  id?: string;
  subject: string;
  predicate: string;
  object: string;
  confidence?: number;
  timestamp?: string;
}

export interface Relationship {
  id: string;
  from_entity: string;
  to_entity: string;
  relationship_type: string;
  properties: Record<string, any>;
}

export interface Query {
  id?: string;
  pattern: QueryPattern;
  constraints?: QueryConstraint[];
  options?: QueryOptions;
}

export interface QueryPattern {
  subject?: string;
  predicate?: string;
  object?: string;
  variables?: string[];
}

export interface QueryConstraint {
  field: string;
  operator: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte' | 'contains' | 'regex';
  value: any;
}

export interface QueryOptions {
  limit?: number;
  offset?: number;
  order_by?: string;
  include_inferred?: boolean;
}

export interface QueryResult {
  success: boolean;
  results: any[];
  count: number;
  execution_time_ms: number;
  error?: string;
}

export interface Rule {
  id: string;
  name: string;
  description?: string;
  conditions: RuleCondition[];
  conclusions: RuleConclusion[];
  confidence: number;
  priority: number;
}

export interface RuleCondition {
  type: 'fact' | 'state' | 'function';
  pattern: any;
  negated?: boolean;
}

export interface RuleConclusion {
  type: 'fact' | 'action' | 'state_change';
  content: any;
  confidence?: number;
}

export interface InferenceResult {
  new_facts: Fact[];
  applied_rules: string[];
  inference_steps: InferenceStep[];
  confidence_scores: Record<string, number>;
}

export interface InferenceStep {
  rule_id: string;
  applied_at: string;
  input_facts: string[];
  output_facts: string[];
}

// Sentiment Analysis Types
export interface SentimentResult {
  overall_sentiment: 'positive' | 'negative' | 'neutral';
  confidence: number;
  scores: {
    positive: number;
    negative: number;
    neutral: number;
  };
  aspects?: AspectSentiment[];
}

export interface AspectSentiment {
  aspect: string;
  sentiment: 'positive' | 'negative' | 'neutral';
  confidence: number;
}

// Preference Extraction Types
export interface PreferenceResult {
  preferences: PreferenceItem[];
  confidence: number;
  categories: string[];
}

export interface PreferenceItem {
  category: string;
  preference: string;
  strength: 'weak' | 'moderate' | 'strong';
  confidence: number;
  context?: string;
}

// Emotion Detection Types
export interface EmotionResult {
  primary_emotion: EmotionType;
  emotions: EmotionScore[];
  intensity: number;
  confidence: number;
}

export interface EmotionScore {
  emotion: EmotionType;
  score: number;
  confidence: number;
}

export type EmotionType = 'joy' | 'sadness' | 'anger' | 'fear' | 'surprise' | 'disgust' | 'trust' | 'anticipation';

// Planning Types
export interface WorldState {
  states: Record<string, StateValue>;
  timestamp?: string;
}

export type StateValue = string | number | boolean | null;

export interface Action {
  id: string;
  name: string;
  description?: string;
  preconditions: ActionPrecondition[];
  effects: ActionEffect[];
  cost: ActionCost;
}

export interface ActionPrecondition {
  state_key: string;
  required_value: StateValue;
  operator?: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte';
}

export interface ActionEffect {
  state_key: string;
  value: StateValue;
  probability?: number;
}

export interface ActionCost {
  base_cost: number;
  variable_costs?: Record<string, number>;
}

export interface Goal {
  id: string;
  name: string;
  description?: string;
  conditions: GoalCondition[];
  priority: GoalPriority;
  deadline?: string;
}

export interface GoalCondition {
  state_key: string;
  target_value: StateValue;
  operator?: 'eq' | 'ne' | 'gt' | 'lt' | 'gte' | 'lte';
  weight?: number;
}

export type GoalPriority = 'low' | 'medium' | 'high' | 'critical';

export interface PlanningResult {
  success: boolean;
  plan?: Plan;
  error?: string;
  search_stats?: {
    nodes_explored: number;
    execution_time_ms: number;
    memory_usage_mb: number;
  };
}

export interface Plan {
  id: string;
  goal_id: string;
  steps: PlanStep[];
  total_cost: number;
  estimated_duration: number;
  created_at: string;
}

export interface PlanStep {
  step_number: number;
  action_id: string;
  description: string;
  estimated_cost: number;
  estimated_duration: number;
  preconditions: Record<string, StateValue>;
  effects: Record<string, StateValue>;
}

export interface ExecutionResult {
  success: boolean;
  executed_steps: PlanStep[];
  final_state: WorldState;
  total_cost: number;
  error?: string;
}

// Memory Management Types
export interface WasmInstanceManager {
  instances: Map<string, any>;
  cleanup(): void;
  getInstance<T>(key: string): T | null;
  setInstance<T>(key: string, instance: T): void;
  removeInstance(key: string): boolean;
}

// Error Types
export class PsychoSymbolicError extends Error {
  constructor(
    message: string,
    public code: string,
    public details?: any
  ) {
    super(message);
    this.name = 'PsychoSymbolicError';
  }
}

export class WasmLoadError extends PsychoSymbolicError {
  constructor(module: string, details?: any) {
    super(`Failed to load WASM module: ${module}`, 'WASM_LOAD_ERROR', details);
  }
}

export class WasmExecutionError extends PsychoSymbolicError {
  constructor(operation: string, details?: any) {
    super(`WASM execution failed: ${operation}`, 'WASM_EXECUTION_ERROR', details);
  }
}

export class InvalidInputError extends PsychoSymbolicError {
  constructor(parameter: string, expected: string, received: any) {
    super(
      `Invalid input for ${parameter}. Expected ${expected}, received ${typeof received}`,
      'INVALID_INPUT_ERROR',
      { parameter, expected, received }
    );
  }
}

// Configuration Types
export interface McpToolConfig {
  enableCaching: boolean;
  cacheSize: number;
  timeoutMs: number;
  maxConcurrentRequests: number;
  logLevel: 'debug' | 'info' | 'warn' | 'error';
}

export interface WasmModuleConfig {
  wasmPath: string;
  initTimeoutMs: number;
  memoryInitialPages: number;
  memoryMaximumPages?: number;
}

export interface PsychoSymbolicConfig {
  mcp: McpToolConfig;
  modules: {
    graphReasoner: WasmModuleConfig;
    textExtractor: WasmModuleConfig;
    plannerSystem: WasmModuleConfig;
  };
}