/**
 * Skills module exports
 *
 * Provides extensible skill system with hot-reload support.
 * Includes built-in Clawdbot-style skills:
 * - Web Search
 * - Memory (Store/Recall)
 * - Code Analysis
 * - Summarization
 */

export { SkillEntity } from '../core/entities/Skill.js';
export type { SkillExecutor as SkillExecutorType, SkillOptions } from '../core/entities/Skill.js';
export type {
  SkillDefinition,
  SkillInput,
  SkillOutput,
  SkillContext,
  SkillResult,
  SkillExample,
} from '../core/types.js';

// Skill Executor - handles skill detection and execution
export {
  SkillExecutor,
  createSkillExecutor,
  type SkillExecutorConfig,
  type SkillMatch,
  type SkillExecutionOptions,
} from './SkillExecutor.js';

// Built-in Skills
export {
  BUILTIN_SKILLS,
  SKILL_CATEGORIES,
  getSkillById,
  getSkillsByCategory,
  WebSearchSkill,
  MemoryStoreSkill,
  MemoryRecallSkill,
  MemorySkills,
  CodeExplainSkill,
  CodeGenerateSkill,
  CodeSkills,
  SummarizeSkill,
  ConversationSummarySkill,
  SummarizeSkills,
} from './builtin/index.js';

export const SKILLS_MODULE_VERSION = '0.2.0';

export interface SkillRegistryOptions {
  builtinSkills?: string[];
  customSkillsDir?: string;
  hotReload?: boolean;
}
