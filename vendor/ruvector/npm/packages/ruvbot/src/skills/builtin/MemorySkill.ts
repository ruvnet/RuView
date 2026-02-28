/**
 * Memory Skill - Store and retrieve information from persistent memory
 *
 * Integrates with RuVector MemoryManager for:
 * - Storing facts, notes, and context
 * - Semantic search across memories
 * - Session and long-term memory management
 */

import type {
  Skill,
  SkillExecutionContext,
  SkillStep,
  SkillExecutionResult,
} from '../../core/skill/index.js';

export const MemoryStoreSkill: Skill = {
  id: 'memory-store',
  name: 'Store Memory',
  description: 'Store information in persistent memory for later recall',
  version: '1.0.0',
  triggers: [
    { type: 'keyword', value: 'remember', confidence: 0.95 },
    { type: 'keyword', value: 'store', confidence: 0.8 },
    { type: 'keyword', value: 'save', confidence: 0.7 },
    { type: 'keyword', value: 'note', confidence: 0.6 },
    { type: 'keyword', value: 'keep in mind', confidence: 0.9 },
    { type: 'intent', value: 'store_memory', confidence: 0.95 },
  ],
  parameters: {
    type: 'object',
    properties: {
      content: {
        type: 'string',
        description: 'The information to remember',
      },
      key: {
        type: 'string',
        description: 'Optional key/label for the memory',
      },
      type: {
        type: 'string',
        description: 'Memory type: episodic, semantic, procedural',
        default: 'semantic',
      },
      tags: {
        type: 'array',
        description: 'Optional tags for categorization',
      },
    },
    required: ['content'],
  },
  execute: memoryStoreExecutor,
};

export const MemoryRecallSkill: Skill = {
  id: 'memory-recall',
  name: 'Recall Memory',
  description: 'Search and retrieve information from memory',
  version: '1.0.0',
  triggers: [
    { type: 'keyword', value: 'recall', confidence: 0.95 },
    { type: 'keyword', value: 'what do you remember', confidence: 0.9 },
    { type: 'keyword', value: 'did i tell you', confidence: 0.8 },
    { type: 'keyword', value: 'do you remember', confidence: 0.85 },
    { type: 'keyword', value: 'what was', confidence: 0.5 },
    { type: 'intent', value: 'recall_memory', confidence: 0.95 },
  ],
  parameters: {
    type: 'object',
    properties: {
      query: {
        type: 'string',
        description: 'The search query for finding memories',
      },
      limit: {
        type: 'number',
        description: 'Maximum number of memories to return',
        default: 5,
      },
      type: {
        type: 'string',
        description: 'Filter by memory type',
      },
    },
    required: ['query'],
  },
  execute: memoryRecallExecutor,
};

async function* memoryStoreExecutor(
  context: SkillExecutionContext,
  params: Record<string, unknown>
): AsyncGenerator<SkillStep, SkillExecutionResult, void> {
  const content = params.content as string;
  const key = params.key as string | undefined;
  const type = (params.type as string) || 'semantic';
  const tags = (params.tags as string[]) || [];

  yield {
    type: 'message',
    content: 'Storing information in memory...',
  };

  yield {
    type: 'progress',
    progress: 50,
  };

  // The actual storage will be handled by the chat handler using memoriesToStore
  const memoryKey = key || `memory-${Date.now()}`;

  yield {
    type: 'progress',
    progress: 100,
  };

  yield {
    type: 'message',
    content: `I'll remember that: "${content.substring(0, 100)}${content.length > 100 ? '...' : ''}"`,
  };

  return {
    success: true,
    output: {
      key: memoryKey,
      content,
      type,
      tags,
    },
    message: 'Memory stored successfully',
    memoriesToStore: [
      {
        content,
        type,
      },
    ],
  };
}

async function* memoryRecallExecutor(
  context: SkillExecutionContext,
  params: Record<string, unknown>
): AsyncGenerator<SkillStep, SkillExecutionResult, void> {
  const query = params.query as string;
  const limit = (params.limit as number) || 5;

  yield {
    type: 'message',
    content: `Searching memories for: "${query}"...`,
  };

  yield {
    type: 'progress',
    progress: 30,
  };

  // Use retrieved memories from context (populated by chat handler)
  const memories = context.retrievedMemories || [];

  yield {
    type: 'progress',
    progress: 100,
  };

  if (memories.length === 0) {
    yield {
      type: 'message',
      content: `I don't have any memories matching "${query}". Would you like to tell me something to remember?`,
    };

    return {
      success: true,
      output: {
        query,
        memories: [],
        count: 0,
      },
      message: 'No memories found',
    };
  }

  // Format memories for display
  const formattedMemories = memories
    .slice(0, limit)
    .map((m, i) => `${i + 1}. ${m.content} (relevance: ${Math.round(m.relevance * 100)}%)`)
    .join('\n');

  yield {
    type: 'message',
    content: `Here's what I remember:\n\n${formattedMemories}`,
  };

  return {
    success: true,
    output: {
      query,
      memories: memories.slice(0, limit),
      count: memories.length,
    },
    message: `Found ${memories.length} relevant memories`,
  };
}

export const MemorySkills = [MemoryStoreSkill, MemoryRecallSkill];
export default MemorySkills;
