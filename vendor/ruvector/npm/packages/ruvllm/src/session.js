"use strict";
/**
 * Session Management for multi-turn conversations
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SessionManager = void 0;
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
class SessionManager {
    constructor(llm) {
        this.sessions = new Map();
        this.llm = llm;
    }
    /**
     * Create a new conversation session
     */
    create(metadata) {
        const id = `session-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        const session = {
            id,
            createdAt: new Date(),
            messageCount: 0,
            messages: [],
            context: [],
            activeMemoryIds: [],
            metadata: metadata ?? {},
        };
        this.sessions.set(id, session);
        return session;
    }
    /**
     * Get session by ID
     */
    get(sessionId) {
        return this.sessions.get(sessionId);
    }
    /**
     * Chat within a session (maintains context)
     */
    chat(sessionId, message, config) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            throw new Error(`Session not found: ${sessionId}`);
        }
        // Add user message
        session.messages.push({
            role: 'user',
            content: message,
            timestamp: new Date(),
        });
        // Build context from recent messages
        const contextWindow = this.buildContext(session);
        // Query with context
        const prompt = contextWindow ? `${contextWindow}\n\nUser: ${message}` : message;
        const response = this.llm.query(prompt, config);
        // Add assistant response
        session.messages.push({
            role: 'assistant',
            content: response.text,
            timestamp: new Date(),
            requestId: response.requestId,
        });
        session.messageCount = session.messages.length;
        return response;
    }
    /**
     * Add system message to session
     */
    addSystemMessage(sessionId, content) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            throw new Error(`Session not found: ${sessionId}`);
        }
        session.messages.push({
            role: 'system',
            content,
            timestamp: new Date(),
        });
        session.messageCount = session.messages.length;
    }
    /**
     * Add context to session (persisted to memory)
     */
    addContext(sessionId, context) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            throw new Error(`Session not found: ${sessionId}`);
        }
        session.context.push(context);
        // Also store in memory for retrieval
        const memoryId = this.llm.addMemory(context, {
            sessionId,
            type: 'context',
            timestamp: new Date().toISOString(),
        });
        session.activeMemoryIds.push(memoryId);
        return memoryId;
    }
    /**
     * Get conversation history
     */
    getHistory(sessionId, limit) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            return [];
        }
        const messages = session.messages;
        return limit ? messages.slice(-limit) : messages;
    }
    /**
     * Clear session history (keep session active)
     */
    clearHistory(sessionId) {
        const session = this.sessions.get(sessionId);
        if (session) {
            session.messages = [];
            session.context = [];
            session.messageCount = 0;
        }
    }
    /**
     * End and delete session
     */
    end(sessionId) {
        return this.sessions.delete(sessionId);
    }
    /**
     * List all active sessions
     */
    list() {
        return Array.from(this.sessions.values());
    }
    /**
     * Export session as JSON
     */
    export(sessionId) {
        const session = this.sessions.get(sessionId);
        if (!session) {
            return null;
        }
        return JSON.stringify(session, null, 2);
    }
    /**
     * Import session from JSON
     */
    import(json) {
        const data = JSON.parse(json);
        const session = {
            ...data,
            createdAt: new Date(data.createdAt),
            messages: data.messages.map((m) => ({
                ...m,
                timestamp: new Date(m.timestamp),
            })),
        };
        this.sessions.set(session.id, session);
        return session;
    }
    /**
     * Build context string from recent messages
     */
    buildContext(session, maxMessages = 10) {
        const recent = session.messages.slice(-maxMessages);
        if (recent.length === 0) {
            return '';
        }
        const contextParts = [];
        // Add persistent context
        if (session.context.length > 0) {
            contextParts.push('Context:\n' + session.context.join('\n'));
        }
        // Add conversation history
        const history = recent
            .map(m => {
            const role = m.role === 'user' ? 'User' : m.role === 'assistant' ? 'Assistant' : 'System';
            return `${role}: ${m.content}`;
        })
            .join('\n');
        if (history) {
            contextParts.push('Conversation:\n' + history);
        }
        return contextParts.join('\n\n');
    }
}
exports.SessionManager = SessionManager;
//# sourceMappingURL=session.js.map