/**
 * Skill Executor - Executes skills and manages skill lifecycle
 *
 * Integrates with:
 * - SkillRegistry for skill lookup
 * - MemoryManager for memory skills
 * - LLM providers for enhanced responses
 */
import type { Skill, SkillExecutionContext, SkillStep, SkillExecutionResult } from '../core/skill/index.js';
interface ISkillRegistry {
    register(skill: Skill): void;
    get(skillId: string): Skill | undefined;
    findByTrigger(input: string): Array<{
        skill: Skill;
        confidence: number;
    }>;
    list(): Skill[];
}
import type { MemoryManager } from '../learning/memory/MemoryManager.js';
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
export declare class SkillExecutor {
    private readonly config;
    private readonly skills;
    constructor(config?: SkillExecutorConfig);
    /**
     * Register a skill
     */
    registerSkill(skill: Skill): void;
    /**
     * Get a skill by ID
     */
    getSkill(id: string): Skill | undefined;
    /**
     * List all registered skills
     */
    listSkills(): Skill[];
    /**
     * Detect skills that match a user message
     */
    detectSkills(message: string): SkillMatch[];
    /**
     * Execute a skill
     */
    executeSkill(skillId: string, options?: SkillExecutionOptions): Promise<{
        steps: SkillStep[];
        result: SkillExecutionResult;
    }>;
    /**
     * Execute a skill and stream results
     */
    streamSkill(skillId: string, options?: SkillExecutionOptions): AsyncGenerator<SkillStep, SkillExecutionResult, void>;
    /**
     * Extract parameters from message based on skill schema
     */
    private extractParams;
    /**
     * Set memory manager (for late binding)
     */
    setMemoryManager(memoryManager: MemoryManager): void;
}
export declare function createSkillExecutor(config?: SkillExecutorConfig): SkillExecutor;
export default SkillExecutor;
//# sourceMappingURL=SkillExecutor.d.ts.map