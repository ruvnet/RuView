/**
 * Core types for the Lean Agentic Learning System
 */

export interface LeanAgenticConfig {
  /** Enable formal verification of actions */
  enableFormalVerification: boolean;

  /** Learning rate for online adaptation */
  learningRate: number;

  /** Maximum planning depth */
  maxPlanningDepth: number;

  /** Confidence threshold for action execution */
  actionThreshold: number;

  /** Enable multi-agent collaboration */
  enableMultiAgent: boolean;

  /** Knowledge graph update frequency */
  kgUpdateFreq: number;
}

export interface Context {
  /** Conversation history */
  history: string[];

  /** User preferences learned over time */
  preferences: Record<string, number>;

  /** Session identifier */
  sessionId: string;

  /** Environment state */
  environment: Record<string, any>;

  /** Timestamp */
  timestamp: number;
}

export interface Action {
  /** Type of action */
  actionType: string;

  /** Human-readable description */
  description: string;

  /** Action parameters */
  parameters: Record<string, string>;

  /** Tool calls required */
  toolCalls: string[];

  /** Expected outcome */
  expectedOutcome?: string;

  /** Expected reward */
  expectedReward: number;
}

export interface Observation {
  /** Whether action succeeded */
  success: boolean;

  /** Result of action */
  result: string;

  /** Changes made */
  changes: string[];

  /** Timestamp */
  timestamp: number;
}

export interface Plan {
  /** Goal being pursued */
  goal: Goal;

  /** Steps to achieve goal */
  steps: PlanStep[];

  /** Estimated total reward */
  estimatedReward: number;

  /** Confidence in plan */
  confidence: number;
}

export interface PlanStep {
  /** Step sequence number */
  sequence: number;

  /** Action to take */
  action: Action;

  /** Preconditions for this step */
  preconditions: string[];

  /** Postconditions after this step */
  postconditions: string[];
}

export interface Goal {
  /** Goal identifier */
  id: string;

  /** Description */
  description: string;

  /** Priority (0-1) */
  priority: number;

  /** Whether achieved */
  achieved: boolean;
}

export interface Theorem {
  /** Theorem identifier */
  id: string;

  /** Mathematical statement */
  statement: string;

  /** Proof (if proven) */
  proof?: Proof;

  /** Confidence score */
  confidence: number;

  /** Tags for categorization */
  tags: string[];
}

export interface Proof {
  /** Proof steps */
  steps: ProofStep[];

  /** Whether proof is valid */
  valid: boolean;

  /** Overall confidence */
  confidence: number;
}

export interface ProofStep {
  /** Inference rule used */
  rule: string;

  /** Premises */
  premises: string[];

  /** Conclusion */
  conclusion: string;

  /** Step confidence */
  confidence: number;
}

export interface Entity {
  /** Entity identifier */
  id: string;

  /** Entity name */
  name: string;

  /** Entity type */
  entityType: EntityType;

  /** Attributes */
  attributes: Record<string, string>;

  /** Confidence score */
  confidence: number;
}

export enum EntityType {
  Person = "Person",
  Place = "Place",
  Organization = "Organization",
  Concept = "Concept",
  Event = "Event",
  Value = "Value",
  Unknown = "Unknown"
}

export interface Relation {
  /** Relation identifier */
  id: string;

  /** Subject entity */
  subject: string;

  /** Predicate/relation type */
  predicate: string;

  /** Object entity */
  object: string;

  /** Confidence score */
  confidence: number;

  /** Source of relation */
  source: string;
}

export interface ProcessingResult {
  /** Action taken */
  action: Action;

  /** Observation received */
  observation: Observation;

  /** Reward earned */
  reward: number;

  /** Whether formally verified */
  verified: boolean;
}

export interface SystemStats {
  /** Total theorems in knowledge base */
  totalTheorems: number;

  /** Total entities in knowledge graph */
  totalEntities: number;

  /** Learning iterations completed */
  learningIterations: number;

  /** Total actions executed */
  totalActions: number;

  /** Average reward */
  averageReward: number;
}

export enum AdaptationStrategy {
  Immediate = "Immediate",
  Batched = "Batched",
  ExperienceReplay = "ExperienceReplay"
}

export interface LearningStats {
  /** Total iterations */
  iterations: number;

  /** Experience buffer size */
  bufferSize: number;

  /** Average reward */
  averageReward: number;

  /** Model parameter count */
  modelParameters: number;
}
