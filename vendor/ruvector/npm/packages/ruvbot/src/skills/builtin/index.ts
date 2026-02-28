/**
 * Built-in Skills Index
 *
 * Exports all built-in skills for RuvBot.
 * These skills provide Clawdbot-style capabilities:
 * - Web search
 * - Memory management
 * - Code analysis
 * - Text summarization
 */

import { WebSearchSkill } from './WebSearchSkill.js';
import { MemoryStoreSkill, MemoryRecallSkill, MemorySkills } from './MemorySkill.js';
import { CodeExplainSkill, CodeGenerateSkill, CodeSkills } from './CodeSkill.js';
import { SummarizeSkill, ConversationSummarySkill, SummarizeSkills } from './SummarizeSkill.js';
import type { Skill } from '../../core/skill/index.js';

/**
 * All built-in skills
 */
export const BUILTIN_SKILLS: Skill[] = [
  // Web Search
  WebSearchSkill,

  // Memory
  MemoryStoreSkill,
  MemoryRecallSkill,

  // Code
  CodeExplainSkill,
  CodeGenerateSkill,

  // Summarization
  SummarizeSkill,
  ConversationSummarySkill,
];

/**
 * Skill categories for organization
 */
export const SKILL_CATEGORIES = {
  search: [WebSearchSkill],
  memory: MemorySkills,
  code: CodeSkills,
  summarize: SummarizeSkills,
};

/**
 * Get skill by ID
 */
export function getSkillById(id: string): Skill | undefined {
  return BUILTIN_SKILLS.find((s) => s.id === id);
}

/**
 * Get skills by category
 */
export function getSkillsByCategory(category: keyof typeof SKILL_CATEGORIES): Skill[] {
  return SKILL_CATEGORIES[category] || [];
}

// Re-export individual skills
export { WebSearchSkill } from './WebSearchSkill.js';
export { MemoryStoreSkill, MemoryRecallSkill, MemorySkills } from './MemorySkill.js';
export { CodeExplainSkill, CodeGenerateSkill, CodeSkills } from './CodeSkill.js';
export { SummarizeSkill, ConversationSummarySkill, SummarizeSkills } from './SummarizeSkill.js';

export default BUILTIN_SKILLS;
