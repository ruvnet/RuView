/**
 * Skill Module - Extensible skill system
 */

export interface Skill {
  id: string;
  name: string;
  description: string;
  version: string;
  triggers: SkillTrigger[];
  parameters: ParameterSchema;
  execute: SkillExecutor;
  hooks?: SkillHooks;
}

export interface SkillTrigger {
  type: 'intent' | 'keyword' | 'pattern' | 'event';
  value: string;
  confidence: number;
}

export interface ParameterSchema {
  type: 'object';
  properties: Record<string, PropertySchema>;
  required?: string[];
}

export interface PropertySchema {
  type: 'string' | 'number' | 'boolean' | 'array' | 'object';
  description?: string;
  default?: unknown;
  enum?: unknown[];
}

export type SkillExecutor = (
  context: SkillExecutionContext,
  params: Record<string, unknown>
) => AsyncGenerator<SkillStep, SkillExecutionResult, void>;

export interface SkillExecutionContext {
  sessionId: string;
  userId: string;
  tenantId: string;
  workspaceId: string;
  conversationHistory: Array<{ role: string; content: string }>;
  retrievedMemories: Array<{ content: string; relevance: number }>;
}

export interface SkillStep {
  type: 'message' | 'action' | 'waiting' | 'progress';
  content?: string;
  action?: string;
  progress?: number;
}

export interface SkillExecutionResult {
  success: boolean;
  output: unknown;
  message?: string;
  memoriesToStore?: Array<{ content: string; type: string }>;
}

export interface SkillHooks {
  beforeExecute?: (context: SkillExecutionContext) => Promise<void>;
  afterExecute?: (result: SkillExecutionResult) => Promise<void>;
  onError?: (error: Error) => Promise<SkillExecutionResult | null>;
}

export class SkillRegistry {
  private skills: Map<string, Skill> = new Map();

  register(skill: Skill): void {
    this.skills.set(skill.id, skill);
  }

  get(skillId: string): Skill | undefined {
    return this.skills.get(skillId);
  }

  findByTrigger(input: string): Array<{ skill: Skill; confidence: number }> {
    const matches: Array<{ skill: Skill; confidence: number }> = [];

    for (const skill of this.skills.values()) {
      for (const trigger of skill.triggers) {
        if (trigger.type === 'keyword' && input.toLowerCase().includes(trigger.value.toLowerCase())) {
          matches.push({ skill, confidence: trigger.confidence });
        }
        // Add more trigger type matching logic here
      }
    }

    return matches.sort((a, b) => b.confidence - a.confidence);
  }

  list(): Skill[] {
    return Array.from(this.skills.values());
  }
}
