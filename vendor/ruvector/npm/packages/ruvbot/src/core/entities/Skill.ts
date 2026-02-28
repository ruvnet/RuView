/**
 * Skill entity - represents an executable skill/capability
 */

import { z } from 'zod';
import type {
  SkillDefinition,
  SkillInput,
  SkillOutput,
  SkillContext,
  SkillResult,
  SkillExample,
} from '../types.js';
import { SkillError } from '../errors.js';

export type SkillExecutor<TInput = unknown, TOutput = unknown> = (
  input: TInput,
  context: SkillContext
) => Promise<SkillResult<TOutput>>;

export interface SkillOptions<TInput = unknown, TOutput = unknown> {
  definition: SkillDefinition;
  executor: SkillExecutor<TInput, TOutput>;
  inputSchema?: z.ZodSchema<TInput>;
  outputSchema?: z.ZodSchema<TOutput>;
}

export class SkillEntity<TInput = unknown, TOutput = unknown> {
  public readonly definition: SkillDefinition;
  private readonly executor: SkillExecutor<TInput, TOutput>;
  private readonly inputSchema?: z.ZodSchema<TInput>;
  private readonly outputSchema?: z.ZodSchema<TOutput>;

  constructor(options: SkillOptions<TInput, TOutput>) {
    this.definition = options.definition;
    this.executor = options.executor;
    this.inputSchema = options.inputSchema;
    this.outputSchema = options.outputSchema;
  }

  /**
   * Get skill name
   */
  get name(): string {
    return this.definition.name;
  }

  /**
   * Get skill description
   */
  get description(): string {
    return this.definition.description;
  }

  /**
   * Get skill version
   */
  get version(): string {
    return this.definition.version;
  }

  /**
   * Get skill inputs
   */
  get inputs(): SkillInput[] {
    return this.definition.inputs;
  }

  /**
   * Get skill outputs
   */
  get outputs(): SkillOutput[] {
    return this.definition.outputs;
  }

  /**
   * Get skill examples
   */
  get examples(): SkillExample[] {
    return this.definition.examples ?? [];
  }

  /**
   * Validate input against schema
   */
  validateInput(input: unknown): { valid: boolean; errors?: string[] } {
    if (!this.inputSchema) {
      return { valid: true };
    }

    try {
      this.inputSchema.parse(input);
      return { valid: true };
    } catch (error) {
      if (error instanceof z.ZodError) {
        return {
          valid: false,
          errors: error.errors.map((e) => `${e.path.join('.')}: ${e.message}`),
        };
      }
      throw error;
    }
  }

  /**
   * Validate output against schema
   */
  validateOutput(output: unknown): { valid: boolean; errors?: string[] } {
    if (!this.outputSchema) {
      return { valid: true };
    }

    try {
      this.outputSchema.parse(output);
      return { valid: true };
    } catch (error) {
      if (error instanceof z.ZodError) {
        return {
          valid: false,
          errors: error.errors.map((e) => `${e.path.join('.')}: ${e.message}`),
        };
      }
      throw error;
    }
  }

  /**
   * Execute the skill
   */
  async execute(input: TInput, context: SkillContext): Promise<SkillResult<TOutput>> {
    const startTime = Date.now();

    // Validate input
    const inputValidation = this.validateInput(input);
    if (!inputValidation.valid) {
      return {
        success: false,
        error: `Input validation failed: ${inputValidation.errors?.join(', ')}`,
        metadata: { latency: Date.now() - startTime },
      };
    }

    try {
      // Execute skill
      const result = await this.executor(input, context);

      // Validate output if successful
      if (result.success && result.data) {
        const outputValidation = this.validateOutput(result.data);
        if (!outputValidation.valid) {
          return {
            success: false,
            error: `Output validation failed: ${outputValidation.errors?.join(', ')}`,
            metadata: { latency: Date.now() - startTime },
          };
        }
      }

      return {
        ...result,
        metadata: {
          ...result.metadata,
          latency: Date.now() - startTime,
        },
      };
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      return {
        success: false,
        error: errorMessage,
        metadata: { latency: Date.now() - startTime },
      };
    }
  }

  /**
   * Generate tool definition for LLM
   */
  toLLMTool(): {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  } {
    const properties: Record<string, unknown> = {};
    const required: string[] = [];

    for (const input of this.definition.inputs) {
      properties[input.name] = {
        type: input.type,
        description: input.description,
      };

      if (input.required) {
        required.push(input.name);
      }
    }

    return {
      name: this.definition.name,
      description: this.definition.description,
      parameters: {
        type: 'object',
        properties,
        required,
      },
    };
  }

  /**
   * Serialize skill definition to JSON
   */
  toJSON(): SkillDefinition {
    return this.definition;
  }

  /**
   * Create a simple skill from a function
   */
  static create<TInput, TOutput>(
    name: string,
    description: string,
    executor: SkillExecutor<TInput, TOutput>,
    options?: {
      inputs?: SkillInput[];
      outputs?: SkillOutput[];
      inputSchema?: z.ZodSchema<TInput>;
      outputSchema?: z.ZodSchema<TOutput>;
    }
  ): SkillEntity<TInput, TOutput> {
    return new SkillEntity({
      definition: {
        name,
        description,
        version: '1.0.0',
        inputs: options?.inputs ?? [],
        outputs: options?.outputs ?? [],
      },
      executor,
      inputSchema: options?.inputSchema,
      outputSchema: options?.outputSchema,
    });
  }
}
