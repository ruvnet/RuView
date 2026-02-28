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
import type { LLMProvider } from '../integration/providers/index.js';
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
    conversationHistory: Array<{
        role: string;
        content: string;
    }>;
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
        tokensUsed?: {
            input: number;
            output: number;
        };
        skillsDetected: number;
        memorySearched: boolean;
    };
}
export declare class ChatEnhancer {
    private readonly config;
    private readonly skillExecutor;
    private readonly memoryManager;
    private llmProvider?;
    constructor(config?: ChatEnhancerConfig);
    /**
     * Set the LLM provider for enhanced responses
     */
    setLLMProvider(provider: LLMProvider): void;
    /**
     * Process a chat message with full enhancement
     */
    processMessage(message: string, context: EnhancedChatContext): Promise<EnhancedChatResponse>;
    /**
     * Store a memory from the conversation
     */
    storeMemory(content: string, tenantId: string, options?: {
        sessionId?: string;
        type?: 'episodic' | 'semantic' | 'procedural' | 'working';
        tags?: string[];
    }): Promise<string>;
    /**
     * Get available skills
     */
    getAvailableSkills(): Array<{
        id: string;
        name: string;
        description: string;
        triggers: string[];
    }>;
    /**
     * Get memory statistics
     */
    getMemoryStats(): {
        totalEntries: number;
        indexedEntries: number;
        tenants: number;
        sessions: number;
    };
    /**
     * Generate proactive assistance hints
     */
    private generateProactiveHints;
}
/**
 * Factory function to create ChatEnhancer
 */
export declare function createChatEnhancer(config?: ChatEnhancerConfig): ChatEnhancer;
export default ChatEnhancer;
//# sourceMappingURL=ChatEnhancer.d.ts.map