/**
 * Session Management for multi-turn conversations
 */
import { ConversationSession, ConversationMessage, QueryResponse, GenerationConfig } from './types';
/**
 * Session Manager for multi-turn conversations
 *
 * @example
 * ```typescript
 * import { RuvLLM, SessionManager } from '@ruvector/ruvllm';
 *
 * const llm = new RuvLLM();
 * const sessions = new SessionManager(llm);
 *
 * // Create a new session
 * const session = sessions.create();
 *
 * // Chat with context
 * const response1 = sessions.chat(session.id, 'What is Python?');
 * const response2 = sessions.chat(session.id, 'How do I install it?');
 * // Second query automatically has context from first
 * ```
 */
export declare class SessionManager {
    private sessions;
    private llm;
    constructor(llm: {
        query: (text: string, config?: GenerationConfig) => QueryResponse;
        addMemory: (content: string, metadata?: Record<string, unknown>) => number;
    });
    /**
     * Create a new conversation session
     */
    create(metadata?: Record<string, unknown>): ConversationSession;
    /**
     * Get session by ID
     */
    get(sessionId: string): ConversationSession | undefined;
    /**
     * Chat within a session (maintains context)
     */
    chat(sessionId: string, message: string, config?: GenerationConfig): QueryResponse;
    /**
     * Add system message to session
     */
    addSystemMessage(sessionId: string, content: string): void;
    /**
     * Add context to session (persisted to memory)
     */
    addContext(sessionId: string, context: string): number;
    /**
     * Get conversation history
     */
    getHistory(sessionId: string, limit?: number): ConversationMessage[];
    /**
     * Clear session history (keep session active)
     */
    clearHistory(sessionId: string): void;
    /**
     * End and delete session
     */
    end(sessionId: string): boolean;
    /**
     * List all active sessions
     */
    list(): ConversationSession[];
    /**
     * Export session as JSON
     */
    export(sessionId: string): string | null;
    /**
     * Import session from JSON
     */
    import(json: string): ConversationSession;
    /**
     * Build context string from recent messages
     */
    private buildContext;
}
//# sourceMappingURL=session.d.ts.map