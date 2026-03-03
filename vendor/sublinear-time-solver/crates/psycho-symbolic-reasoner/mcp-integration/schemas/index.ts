import { z } from 'zod';

// Common schemas
const StateValueSchema = z.union([
  z.string(),
  z.number(),
  z.boolean(),
  z.null()
]);

const UuidSchema = z.string().uuid();
const NonEmptyStringSchema = z.string().min(1);
const PositiveNumberSchema = z.number().positive();
const NonNegativeNumberSchema = z.number().min(0);

// Graph Reasoner Schemas
export const EntitySchema = z.object({
  id: NonEmptyStringSchema,
  name: NonEmptyStringSchema,
  attributes: z.record(z.any()).optional().default({})
});

export const FactSchema = z.object({
  id: NonEmptyStringSchema.optional(),
  subject: NonEmptyStringSchema,
  predicate: NonEmptyStringSchema,
  object: NonEmptyStringSchema,
  confidence: z.number().min(0).max(1).optional().default(1.0),
  timestamp: z.string().optional()
});

export const RelationshipSchema = z.object({
  id: NonEmptyStringSchema,
  from_entity: NonEmptyStringSchema,
  to_entity: NonEmptyStringSchema,
  relationship_type: NonEmptyStringSchema,
  properties: z.record(z.any()).optional().default({})
});

export const QueryConstraintSchema = z.object({
  field: NonEmptyStringSchema,
  operator: z.enum(['eq', 'ne', 'gt', 'lt', 'gte', 'lte', 'contains', 'regex']),
  value: z.any()
});

export const QueryOptionsSchema = z.object({
  limit: PositiveNumberSchema.optional(),
  offset: NonNegativeNumberSchema.optional().default(0),
  order_by: NonEmptyStringSchema.optional(),
  include_inferred: z.boolean().optional().default(false)
});

export const QueryPatternSchema = z.object({
  subject: NonEmptyStringSchema.optional(),
  predicate: NonEmptyStringSchema.optional(),
  object: NonEmptyStringSchema.optional(),
  variables: z.array(NonEmptyStringSchema).optional().default([])
});

export const QuerySchema = z.object({
  id: NonEmptyStringSchema.optional(),
  pattern: QueryPatternSchema,
  constraints: z.array(QueryConstraintSchema).optional().default([]),
  options: QueryOptionsSchema.optional().default({})
});

export const RuleConditionSchema = z.object({
  type: z.enum(['fact', 'state', 'function']),
  pattern: z.any(),
  negated: z.boolean().optional().default(false)
});

export const RuleConclusionSchema = z.object({
  type: z.enum(['fact', 'action', 'state_change']),
  content: z.any(),
  confidence: z.number().min(0).max(1).optional().default(1.0)
});

export const RuleSchema = z.object({
  id: NonEmptyStringSchema,
  name: NonEmptyStringSchema,
  description: z.string().optional(),
  conditions: z.array(RuleConditionSchema).min(1),
  conclusions: z.array(RuleConclusionSchema).min(1),
  confidence: z.number().min(0).max(1).default(1.0),
  priority: NonNegativeNumberSchema.default(0)
});

// Text Extractor Schemas
export const SentimentAnalysisRequestSchema = z.object({
  text: NonEmptyStringSchema,
  options: z.object({
    include_aspects: z.boolean().optional().default(false),
    language: z.string().optional().default('en'),
    confidence_threshold: z.number().min(0).max(1).optional().default(0.5)
  }).optional().default({})
});

export const PreferenceExtractionRequestSchema = z.object({
  text: NonEmptyStringSchema,
  options: z.object({
    categories: z.array(NonEmptyStringSchema).optional(),
    min_confidence: z.number().min(0).max(1).optional().default(0.3),
    max_preferences: PositiveNumberSchema.optional().default(10)
  }).optional().default({})
});

export const EmotionDetectionRequestSchema = z.object({
  text: NonEmptyStringSchema,
  options: z.object({
    emotion_model: z.enum(['plutchik', 'ekman', 'custom']).optional().default('plutchik'),
    intensity_threshold: z.number().min(0).max(1).optional().default(0.2),
    include_secondary: z.boolean().optional().default(true)
  }).optional().default({})
});

export const TextAnalysisRequestSchema = z.object({
  text: NonEmptyStringSchema,
  include_sentiment: z.boolean().optional().default(true),
  include_preferences: z.boolean().optional().default(true),
  include_emotions: z.boolean().optional().default(true),
  options: z.object({
    sentiment: SentimentAnalysisRequestSchema.shape.options.optional(),
    preferences: PreferenceExtractionRequestSchema.shape.options.optional(),
    emotions: EmotionDetectionRequestSchema.shape.options.optional()
  }).optional().default({})
});

// Planner Schemas
export const WorldStateSchema = z.object({
  states: z.record(StateValueSchema),
  timestamp: z.string().optional()
});

export const ActionPreconditionSchema = z.object({
  state_key: NonEmptyStringSchema,
  required_value: StateValueSchema,
  operator: z.enum(['eq', 'ne', 'gt', 'lt', 'gte', 'lte']).optional().default('eq')
});

export const ActionEffectSchema = z.object({
  state_key: NonEmptyStringSchema,
  value: StateValueSchema,
  probability: z.number().min(0).max(1).optional().default(1.0)
});

export const ActionCostSchema = z.object({
  base_cost: NonNegativeNumberSchema,
  variable_costs: z.record(z.number()).optional().default({})
});

export const ActionSchema = z.object({
  id: NonEmptyStringSchema,
  name: NonEmptyStringSchema,
  description: z.string().optional(),
  preconditions: z.array(ActionPreconditionSchema).default([]),
  effects: z.array(ActionEffectSchema).min(1),
  cost: ActionCostSchema
});

export const GoalConditionSchema = z.object({
  state_key: NonEmptyStringSchema,
  target_value: StateValueSchema,
  operator: z.enum(['eq', 'ne', 'gt', 'lt', 'gte', 'lte']).optional().default('eq'),
  weight: PositiveNumberSchema.optional().default(1.0)
});

export const GoalSchema = z.object({
  id: NonEmptyStringSchema,
  name: NonEmptyStringSchema,
  description: z.string().optional(),
  conditions: z.array(GoalConditionSchema).min(1),
  priority: z.enum(['low', 'medium', 'high', 'critical']).default('medium'),
  deadline: z.string().optional()
});

export const PlanningRequestSchema = z.object({
  goal_id: NonEmptyStringSchema.optional(),
  target_state: WorldStateSchema.optional(),
  options: z.object({
    max_depth: PositiveNumberSchema.optional().default(10),
    timeout_ms: PositiveNumberSchema.optional().default(30000),
    heuristic: z.enum(['astar', 'dijkstra', 'greedy']).optional().default('astar'),
    allow_partial_plans: z.boolean().optional().default(false)
  }).optional().default({})
}).refine(
  (data) => data.goal_id || data.target_state,
  {
    message: "Either goal_id or target_state must be provided",
    path: ["goal_id"]
  }
);

export const StateUpdateSchema = z.object({
  key: NonEmptyStringSchema,
  value: StateValueSchema
});

export const PlanExecutionRequestSchema = z.object({
  plan_json: NonEmptyStringSchema,
  options: z.object({
    dry_run: z.boolean().optional().default(false),
    stop_on_failure: z.boolean().optional().default(true),
    validate_preconditions: z.boolean().optional().default(true)
  }).optional().default({})
});

// MCP Tool Schemas
export const QueryGraphToolSchema = z.object({
  query: QuerySchema,
  options: z.object({
    format: z.enum(['json', 'turtle', 'csv']).optional().default('json'),
    include_metadata: z.boolean().optional().default(true)
  }).optional().default({})
});

export const AddFactToolSchema = z.object({
  fact: FactSchema,
  options: z.object({
    validate: z.boolean().optional().default(true),
    auto_infer: z.boolean().optional().default(false)
  }).optional().default({})
});

export const AddRuleToolSchema = z.object({
  rule: RuleSchema,
  options: z.object({
    validate: z.boolean().optional().default(true),
    replace_existing: z.boolean().optional().default(false)
  }).optional().default({})
});

export const InferenceToolSchema = z.object({
  max_iterations: PositiveNumberSchema.optional().default(10),
  options: z.object({
    confidence_threshold: z.number().min(0).max(1).optional().default(0.5),
    max_new_facts: PositiveNumberSchema.optional().default(1000),
    timeout_ms: PositiveNumberSchema.optional().default(30000)
  }).optional().default({})
});

// Validation helpers
export const validateInput = <T>(schema: z.ZodSchema<T>, data: unknown): T => {
  try {
    return schema.parse(data);
  } catch (error) {
    if (error instanceof z.ZodError) {
      const issues = error.issues.map(issue => 
        `${issue.path.join('.')}: ${issue.message}`
      ).join(', ');
      throw new Error(`Validation failed: ${issues}`);
    }
    throw error;
  }
};

export const createSafeValidator = <T>(schema: z.ZodSchema<T>) => {
  return (data: unknown): { success: true; data: T } | { success: false; error: string } => {
    try {
      const validated = schema.parse(data);
      return { success: true, data: validated };
    } catch (error) {
      if (error instanceof z.ZodError) {
        const issues = error.issues.map(issue => 
          `${issue.path.join('.')}: ${issue.message}`
        ).join(', ');
        return { success: false, error: `Validation failed: ${issues}` };
      }
      return { success: false, error: 'Unknown validation error' };
    }
  };
};

// Export all schemas for external use
export const schemas = {
  // Graph Reasoner
  Entity: EntitySchema,
  Fact: FactSchema,
  Relationship: RelationshipSchema,
  Query: QuerySchema,
  QueryPattern: QueryPatternSchema,
  QueryConstraint: QueryConstraintSchema,
  QueryOptions: QueryOptionsSchema,
  Rule: RuleSchema,
  RuleCondition: RuleConditionSchema,
  RuleConclusion: RuleConclusionSchema,
  
  // Text Extractor
  SentimentAnalysisRequest: SentimentAnalysisRequestSchema,
  PreferenceExtractionRequest: PreferenceExtractionRequestSchema,
  EmotionDetectionRequest: EmotionDetectionRequestSchema,
  TextAnalysisRequest: TextAnalysisRequestSchema,
  
  // Planner
  WorldState: WorldStateSchema,
  Action: ActionSchema,
  ActionPrecondition: ActionPreconditionSchema,
  ActionEffect: ActionEffectSchema,
  ActionCost: ActionCostSchema,
  Goal: GoalSchema,
  GoalCondition: GoalConditionSchema,
  PlanningRequest: PlanningRequestSchema,
  StateUpdate: StateUpdateSchema,
  PlanExecutionRequest: PlanExecutionRequestSchema,
  
  // MCP Tools
  QueryGraphTool: QueryGraphToolSchema,
  AddFactTool: AddFactToolSchema,
  AddRuleTool: AddRuleToolSchema,
  InferenceTool: InferenceToolSchema
};

export type EntityType = z.infer<typeof EntitySchema>;
export type FactType = z.infer<typeof FactSchema>;
export type RelationshipType = z.infer<typeof RelationshipSchema>;
export type QueryType = z.infer<typeof QuerySchema>;
export type RuleType = z.infer<typeof RuleSchema>;
export type SentimentAnalysisRequestType = z.infer<typeof SentimentAnalysisRequestSchema>;
export type PreferenceExtractionRequestType = z.infer<typeof PreferenceExtractionRequestSchema>;
export type EmotionDetectionRequestType = z.infer<typeof EmotionDetectionRequestSchema>;
export type TextAnalysisRequestType = z.infer<typeof TextAnalysisRequestSchema>;
export type WorldStateType = z.infer<typeof WorldStateSchema>;
export type ActionType = z.infer<typeof ActionSchema>;
export type GoalType = z.infer<typeof GoalSchema>;
export type PlanningRequestType = z.infer<typeof PlanningRequestSchema>;
export type StateUpdateType = z.infer<typeof StateUpdateSchema>;
export type PlanExecutionRequestType = z.infer<typeof PlanExecutionRequestSchema>;