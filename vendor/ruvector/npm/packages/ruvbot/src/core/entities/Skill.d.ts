/**
 * Skill entity - represents an executable skill/capability
 */
import { z } from 'zod';
import type { SkillDefinition, SkillInput, SkillOutput, SkillContext, SkillResult, SkillExample } from '../types.js';
export type SkillExecutor<TInput = unknown, TOutput = unknown> = (input: TInput, context: SkillContext) => Promise<SkillResult<TOutput>>;
export interface SkillOptions<TInput = unknown, TOutput = unknown> {
    definition: SkillDefinition;
    executor: SkillExecutor<TInput, TOutput>;
    inputSchema?: z.ZodSchema<TInput>;
    outputSchema?: z.ZodSchema<TOutput>;
}
export declare class SkillEntity<TInput = unknown, TOutput = unknown> {
    readonly definition: SkillDefinition;
    private readonly executor;
    private readonly inputSchema?;
    private readonly outputSchema?;
    constructor(options: SkillOptions<TInput, TOutput>);
    /**
     * Get skill name
     */
    get name(): string;
    /**
     * Get skill description
     */
    get description(): string;
    /**
     * Get skill version
     */
    get version(): string;
    /**
     * Get skill inputs
     */
    get inputs(): SkillInput[];
    /**
     * Get skill outputs
     */
    get outputs(): SkillOutput[];
    /**
     * Get skill examples
     */
    get examples(): SkillExample[];
    /**
     * Validate input against schema
     */
    validateInput(input: unknown): {
        valid: boolean;
        errors?: string[];
    };
    /**
     * Validate output against schema
     */
    validateOutput(output: unknown): {
        valid: boolean;
        errors?: string[];
    };
    /**
     * Execute the skill
     */
    execute(input: TInput, context: SkillContext): Promise<SkillResult<TOutput>>;
    /**
     * Generate tool definition for LLM
     */
    toLLMTool(): {
        name: string;
        description: string;
        parameters: Record<string, unknown>;
    };
    /**
     * Serialize skill definition to JSON
     */
    toJSON(): SkillDefinition;
    /**
     * Create a simple skill from a function
     */
    static create<TInput, TOutput>(name: string, description: string, executor: SkillExecutor<TInput, TOutput>, options?: {
        inputs?: SkillInput[];
        outputs?: SkillOutput[];
        inputSchema?: z.ZodSchema<TInput>;
        outputSchema?: z.ZodSchema<TOutput>;
    }): SkillEntity<TInput, TOutput>;
}
//# sourceMappingURL=Skill.d.ts.map