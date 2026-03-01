/**
 * ChatEnhancer - Enhanced chat processing with skills, memory, and proactive assistance
 *
 * This is the core component that makes RuvBot an ultimate chatbot by integrating:
 * - Skill detection and execution
 * - Memory search and storage
 * - Proactive assistance suggestions
 * - Context-aware responses
 * - WASM embeddings (when available)
 */

import { SkillExecutor, type SkillMatch } from '../skills/SkillExecutor.js';
import { MemoryManager } from '../learning/memory/MemoryManager.js';
import type { LLMProvider } from '../integration/providers/index.js';
import type { SkillStep } from './skill/index.js';

export interface ChatEnhancerConfig {
  enableSkills?: boolean;
  enableMemory?: boolean;
  enableProactiveAssistance?: boolean;
  memorySearchThreshold?: number;
  memorySearchLimit?: number;
  skillConfidenceThreshold?: number;
  tenantId?: string;
}

export interface EnhancedChatContext {
  sessionId: string;
  userId: string;
  tenantId: string;
  conversationHistory: Array<{ role: string; content: string }>;
  metadata?: Record<string, unknown>;
}

export interface EnhancedChatResponse {
  content: string;
  skillsUsed?: Array<{
    skillId: string;
    skillName: string;
    success: boolean;
    output?: unknown;
  }>;
  memoriesRecalled?: Array<{
    content: string;
    relevance: number;
  }>;
  memoriesStored?: number;
  proactiveHints?: string[];
  metadata?: {
    processingTime: number;
    tokensUsed?: { input: number; output: number };
    skillsDetected: number;
    memorySearched: boolean;
  };
}

export class ChatEnhancer {
  private readonly config: Required<ChatEnhancerConfig>;
  private readonly skillExecutor: SkillExecutor;
  private readonly memoryManager: MemoryManager;
  private llmProvider?: LLMProvider;

  constructor(config: ChatEnhancerConfig = {}) {
    this.config = {
      enableSkills: config.enableSkills ?? true,
      enableMemory: config.enableMemory ?? true,
      enableProactiveAssistance: config.enableProactiveAssistance ?? true,
      memorySearchThreshold: config.memorySearchThreshold ?? 0.5,
      memorySearchLimit: config.memorySearchLimit ?? 5,
      skillConfidenceThreshold: config.skillConfidenceThreshold ?? 0.6,
      tenantId: config.tenantId ?? 'default',
    };

    this.memoryManager = new MemoryManager({
      dimension: 384,
      maxEntries: 100000,
    });

    this.skillExecutor = new SkillExecutor({
      enableBuiltinSkills: this.config.enableSkills,
      memoryManager: this.memoryManager,
      tenantId: this.config.tenantId,
    });
  }

  /**
   * Set the LLM provider for enhanced responses
   */
  setLLMProvider(provider: LLMProvider): void {
    this.llmProvider = provider;
  }

  /**
   * Process a chat message with full enhancement
   */
  async processMessage(
    message: string,
    context: EnhancedChatContext
  ): Promise<EnhancedChatResponse> {
    const startTime = Date.now();
    const response: EnhancedChatResponse = {
      content: '',
      skillsUsed: [],
      memoriesRecalled: [],
      memoriesStored: 0,
      proactiveHints: [],
      metadata: {
        processingTime: 0,
        skillsDetected: 0,
        memorySearched: false,
      },
    };

    // Step 1: Detect skills
    let detectedSkills: SkillMatch[] = [];
    if (this.config.enableSkills) {
      detectedSkills = this.skillExecutor.detectSkills(message);
      response.metadata!.skillsDetected = detectedSkills.length;
    }

    // Step 2: Search memory for context
    let relevantMemories: Array<{ content: string; relevance: number }> = [];
    if (this.config.enableMemory) {
      try {
        // Simple text-based memory search (no embeddings yet)
        const memories = await this.memoryManager.listByTenant(context.tenantId, 100);
        relevantMemories = memories
          .filter((m) => {
            const content = String(m.value).toLowerCase();
            const query = message.toLowerCase();
            // Simple keyword matching
            const words = query.split(/\s+/).filter((w) => w.length > 3);
            return words.some((w) => content.includes(w));
          })
          .map((m) => ({
            content: String(m.value),
            relevance: 0.7, // Placeholder relevance
          }))
          .slice(0, this.config.memorySearchLimit);

        response.memoriesRecalled = relevantMemories;
        response.metadata!.memorySearched = true;
      } catch (error) {
        console.warn('Memory search failed:', error);
      }
    }

    // Step 3: Execute high-confidence skills
    const skillResponses: string[] = [];
    for (const match of detectedSkills) {
      if (match.confidence >= this.config.skillConfidenceThreshold) {
        try {
          const { steps, result } = await this.skillExecutor.executeSkill(match.skill.id, {
            params: match.params,
            context: {
              sessionId: context.sessionId,
              userId: context.userId,
              tenantId: context.tenantId,
              conversationHistory: context.conversationHistory,
              retrievedMemories: relevantMemories,
            },
          });

          // Collect skill messages
          const messages = steps
            .filter((s): s is SkillStep & { content: string } => s.type === 'message' && !!s.content)
            .map((s) => s.content);

          if (messages.length > 0) {
            skillResponses.push(messages.join('\n'));
          }

          response.skillsUsed!.push({
            skillId: match.skill.id,
            skillName: match.skill.name,
            success: result.success,
            output: result.output,
          });

          // Count stored memories
          if (result.memoriesToStore) {
            response.memoriesStored! += result.memoriesToStore.length;
          }
        } catch (error) {
          console.warn(`Skill ${match.skill.id} execution failed:`, error);
          response.skillsUsed!.push({
            skillId: match.skill.id,
            skillName: match.skill.name,
            success: false,
          });
        }
      }
    }

    // Step 4: Build enhanced response
    if (skillResponses.length > 0) {
      response.content = skillResponses.join('\n\n---\n\n');
    }

    // Step 5: Generate proactive hints
    if (this.config.enableProactiveAssistance) {
      response.proactiveHints = this.generateProactiveHints(message, detectedSkills);
    }

    // Calculate processing time
    response.metadata!.processingTime = Date.now() - startTime;

    return response;
  }

  /**
   * Store a memory from the conversation
   */
  async storeMemory(
    content: string,
    tenantId: string,
    options?: {
      sessionId?: string;
      type?: 'episodic' | 'semantic' | 'procedural' | 'working';
      tags?: string[];
    }
  ): Promise<string> {
    const entry = await this.memoryManager.store(
      tenantId,
      `memory-${Date.now()}`,
      content,
      {
        sessionId: options?.sessionId,
        type: options?.type || 'semantic',
        text: content,
        tags: options?.tags || [],
      }
    );
    return entry.id;
  }

  /**
   * Get available skills
   */
  getAvailableSkills(): Array<{
    id: string;
    name: string;
    description: string;
    triggers: string[];
  }> {
    return this.skillExecutor.listSkills().map((skill) => ({
      id: skill.id,
      name: skill.name,
      description: skill.description,
      triggers: skill.triggers
        .filter((t) => t.type === 'keyword')
        .map((t) => t.value),
    }));
  }

  /**
   * Get memory statistics
   */
  getMemoryStats(): {
    totalEntries: number;
    indexedEntries: number;
    tenants: number;
    sessions: number;
  } {
    return this.memoryManager.stats();
  }

  /**
   * Generate proactive assistance hints
   */
  private generateProactiveHints(message: string, detectedSkills: SkillMatch[]): string[] {
    const hints: string[] = [];
    const lowerMessage = message.toLowerCase();

    // Suggest related skills
    const usedSkillIds = new Set(detectedSkills.map((s) => s.skill.id));

    // If asking about code, suggest code skills
    if (lowerMessage.includes('code') && !usedSkillIds.has('code-explain')) {
      hints.push('I can also explain or generate code. Try: "explain this code" or "write code for..."');
    }

    // If searching, suggest memory
    if ((lowerMessage.includes('search') || lowerMessage.includes('find')) && !usedSkillIds.has('memory-recall')) {
      hints.push('I can also search my memory for past conversations. Try: "do you remember..."');
    }

    // If long text, suggest summarization
    if (message.length > 500 && !usedSkillIds.has('summarize')) {
      hints.push('That\'s a lot of text! I can summarize it for you. Try: "summarize this"');
    }

    // General capability hints
    if (detectedSkills.length === 0) {
      const randomHints = [
        'I can search the web, remember facts, analyze code, and summarize text.',
        'Try asking me to "search for..." or "remember that..."',
        'I have skills for web search, memory, code analysis, and summarization.',
      ];
      hints.push(randomHints[Math.floor(Math.random() * randomHints.length)]);
    }

    return hints.slice(0, 2); // Max 2 hints
  }
}

/**
 * Factory function to create ChatEnhancer
 */
export function createChatEnhancer(config?: ChatEnhancerConfig): ChatEnhancer {
  return new ChatEnhancer(config);
}

export default ChatEnhancer;
