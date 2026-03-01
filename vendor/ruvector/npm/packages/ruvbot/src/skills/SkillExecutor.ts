/**
 * Skill Executor - Executes skills and manages skill lifecycle
 *
 * Integrates with:
 * - SkillRegistry for skill lookup
 * - MemoryManager for memory skills
 * - LLM providers for enhanced responses
 */

import type {
  Skill,
  SkillExecutionContext,
  SkillStep,
  SkillExecutionResult,
} from '../core/skill/index.js';

// Internal skill registry interface
interface ISkillRegistry {
  register(skill: Skill): void;
  get(skillId: string): Skill | undefined;
  findByTrigger(input: string): Array<{ skill: Skill; confidence: number }>;
  list(): Skill[];
}
import type { MemoryManager } from '../learning/memory/MemoryManager.js';
import { BUILTIN_SKILLS, getSkillById } from './builtin/index.js';

export interface SkillExecutorConfig {
  enableBuiltinSkills?: boolean;
  skillRegistry?: ISkillRegistry;
  memoryManager?: MemoryManager;
  tenantId?: string;
}

export interface SkillMatch {
  skill: Skill;
  confidence: number;
  params?: Record<string, unknown>;
}

export interface SkillExecutionOptions {
  params?: Record<string, unknown>;
  context?: Partial<SkillExecutionContext>;
  timeout?: number;
}

interface ResolvedSkillExecutorConfig {
  enableBuiltinSkills: boolean;
  skillRegistry: ISkillRegistry;
  memoryManager: MemoryManager | undefined;
  tenantId: string;
}

export class SkillExecutor {
  private readonly config: ResolvedSkillExecutorConfig;
  private readonly skills: Map<string, Skill> = new Map();

  constructor(config: SkillExecutorConfig = {}) {
    this.config = {
      enableBuiltinSkills: config.enableBuiltinSkills ?? true,
      skillRegistry: config.skillRegistry ?? new SkillRegistryImpl(),
      memoryManager: config.memoryManager,
      tenantId: config.tenantId ?? 'default',
    };

    // Register built-in skills
    if (this.config.enableBuiltinSkills) {
      for (const skill of BUILTIN_SKILLS) {
        this.registerSkill(skill);
      }
    }
  }

  /**
   * Register a skill
   */
  registerSkill(skill: Skill): void {
    this.skills.set(skill.id, skill);
    if (this.config.skillRegistry) {
      this.config.skillRegistry.register(skill);
    }
  }

  /**
   * Get a skill by ID
   */
  getSkill(id: string): Skill | undefined {
    return this.skills.get(id) || getSkillById(id);
  }

  /**
   * List all registered skills
   */
  listSkills(): Skill[] {
    return Array.from(this.skills.values());
  }

  /**
   * Detect skills that match a user message
   */
  detectSkills(message: string): SkillMatch[] {
    const matches: SkillMatch[] = [];
    const lowerMessage = message.toLowerCase();

    for (const skill of this.skills.values()) {
      for (const trigger of skill.triggers) {
        let matched = false;
        let confidence = trigger.confidence;

        switch (trigger.type) {
          case 'keyword':
            if (lowerMessage.includes(trigger.value.toLowerCase())) {
              matched = true;
            }
            break;

          case 'pattern':
            try {
              const regex = new RegExp(trigger.value, 'i');
              if (regex.test(message)) {
                matched = true;
              }
            } catch {
              // Invalid regex, skip
            }
            break;

          case 'intent':
            // Intent matching would require NLU - for now, use keyword fallback
            break;

          case 'event':
            // Event triggers are handled separately
            break;
        }

        if (matched) {
          // Check if skill already matched with higher confidence
          const existing = matches.find((m) => m.skill.id === skill.id);
          if (existing) {
            if (confidence > existing.confidence) {
              existing.confidence = confidence;
            }
          } else {
            matches.push({
              skill,
              confidence,
              params: this.extractParams(message, skill),
            });
          }
        }
      }
    }

    // Sort by confidence
    return matches.sort((a, b) => b.confidence - a.confidence);
  }

  /**
   * Execute a skill
   */
  async executeSkill(
    skillId: string,
    options: SkillExecutionOptions = {}
  ): Promise<{
    steps: SkillStep[];
    result: SkillExecutionResult;
  }> {
    const skill = this.getSkill(skillId);
    if (!skill) {
      throw new Error(`Skill not found: ${skillId}`);
    }

    const context: SkillExecutionContext = {
      sessionId: options.context?.sessionId || '',
      userId: options.context?.userId || '',
      tenantId: options.context?.tenantId || this.config.tenantId,
      workspaceId: options.context?.workspaceId || 'default',
      conversationHistory: options.context?.conversationHistory || [],
      retrievedMemories: options.context?.retrievedMemories || [],
    };

    const params = options.params || {};
    const steps: SkillStep[] = [];

    // Call beforeExecute hook
    if (skill.hooks?.beforeExecute) {
      await skill.hooks.beforeExecute(context);
    }

    try {
      // Execute skill generator
      const generator = skill.execute(context, params);

      for await (const step of generator) {
        steps.push(step);
      }

      // Get the final result (returned from generator)
      const result = await (async () => {
        let lastValue: SkillExecutionResult | undefined;
        const gen = skill.execute(context, params);
        let next = await gen.next();
        while (!next.done) {
          next = await gen.next();
        }
        return next.value as SkillExecutionResult;
      })();

      // Call afterExecute hook
      if (skill.hooks?.afterExecute) {
        await skill.hooks.afterExecute(result);
      }

      // Store memories if requested
      if (result.memoriesToStore && this.config.memoryManager) {
        for (const memory of result.memoriesToStore) {
          await this.config.memoryManager.store(
            context.tenantId,
            `skill-${skillId}-${Date.now()}`,
            memory.content,
            {
              sessionId: context.sessionId,
              type: memory.type as any,
              text: memory.content,
              tags: ['skill', skillId],
            }
          );
        }
      }

      return { steps, result };
    } catch (error) {
      const errorResult: SkillExecutionResult = {
        success: false,
        output: { error: (error as Error).message },
        message: `Skill execution failed: ${(error as Error).message}`,
      };

      // Call onError hook
      if (skill.hooks?.onError) {
        const recovered = await skill.hooks.onError(error as Error);
        if (recovered) {
          return { steps, result: recovered };
        }
      }

      return { steps, result: errorResult };
    }
  }

  /**
   * Execute a skill and stream results
   */
  async *streamSkill(
    skillId: string,
    options: SkillExecutionOptions = {}
  ): AsyncGenerator<SkillStep, SkillExecutionResult, void> {
    const skill = this.getSkill(skillId);
    if (!skill) {
      throw new Error(`Skill not found: ${skillId}`);
    }

    const context: SkillExecutionContext = {
      sessionId: options.context?.sessionId || '',
      userId: options.context?.userId || '',
      tenantId: options.context?.tenantId || this.config.tenantId,
      workspaceId: options.context?.workspaceId || 'default',
      conversationHistory: options.context?.conversationHistory || [],
      retrievedMemories: options.context?.retrievedMemories || [],
    };

    const params = options.params || {};

    // Execute skill generator and yield steps
    const generator = skill.execute(context, params);

    let result: SkillExecutionResult;

    for await (const step of generator) {
      yield step;
    }

    // Return final result
    const finalRun = skill.execute(context, params);
    let next = await finalRun.next();
    while (!next.done) {
      next = await finalRun.next();
    }

    return next.value as SkillExecutionResult;
  }

  /**
   * Extract parameters from message based on skill schema
   */
  private extractParams(message: string, skill: Skill): Record<string, unknown> {
    const params: Record<string, unknown> = {};
    const schema = skill.parameters;

    // Simple extraction based on common patterns
    for (const [name, prop] of Object.entries(schema.properties)) {
      switch (prop.type) {
        case 'string':
          // For query/text/content params, use the whole message or extract quoted text
          if (name === 'query' || name === 'text' || name === 'content' || name === 'description') {
            // Try to extract quoted text first
            const quotedMatch = message.match(/"([^"]+)"/);
            if (quotedMatch) {
              params[name] = quotedMatch[1];
            } else {
              // Use message after skill trigger words
              const words = message.split(/\s+/);
              const triggerIndex = words.findIndex((w) =>
                skill.triggers.some((t) =>
                  t.type === 'keyword' && w.toLowerCase().includes(t.value.toLowerCase())
                )
              );
              if (triggerIndex >= 0 && triggerIndex < words.length - 1) {
                let extracted = words.slice(triggerIndex + 1).join(' ');
                // Strip common prepositions from the start (for, about, on, the, a, an)
                extracted = extracted.replace(/^(for|about|on|the|a|an)\s+/i, '');
                params[name] = extracted;
              } else {
                params[name] = message;
              }
            }
          }
          // Extract code blocks
          else if (name === 'code') {
            const codeMatch = message.match(/```[\w]*\n?([\s\S]*?)```/);
            if (codeMatch) {
              params[name] = codeMatch[1].trim();
            }
          }
          break;

        case 'number':
          const numMatch = message.match(/\b(\d+)\b/);
          if (numMatch && prop.default === undefined) {
            params[name] = parseInt(numMatch[1], 10);
          } else if (prop.default !== undefined) {
            params[name] = prop.default;
          }
          break;

        case 'boolean':
          if (prop.default !== undefined) {
            params[name] = prop.default;
          }
          break;
      }
    }

    return params;
  }

  /**
   * Set memory manager (for late binding)
   */
  setMemoryManager(memoryManager: MemoryManager): void {
    (this.config as any).memoryManager = memoryManager;
  }
}

/**
 * Simple skill registry implementation
 */
class SkillRegistryImpl implements ISkillRegistry {
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
          break;
        }
      }
    }

    return matches.sort((a, b) => b.confidence - a.confidence);
  }

  list(): Skill[] {
    return Array.from(this.skills.values());
  }
}

export function createSkillExecutor(config?: SkillExecutorConfig): SkillExecutor {
  return new SkillExecutor(config);
}

export default SkillExecutor;
