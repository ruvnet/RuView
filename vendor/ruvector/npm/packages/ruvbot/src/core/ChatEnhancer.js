"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.ChatEnhancer = void 0;
exports.createChatEnhancer = createChatEnhancer;
const SkillExecutor_js_1 = require("../skills/SkillExecutor.js");
const MemoryManager_js_1 = require("../learning/memory/MemoryManager.js");
class ChatEnhancer {
    constructor(config = {}) {
        this.config = {
            enableSkills: config.enableSkills ?? true,
            enableMemory: config.enableMemory ?? true,
            enableProactiveAssistance: config.enableProactiveAssistance ?? true,
            memorySearchThreshold: config.memorySearchThreshold ?? 0.5,
            memorySearchLimit: config.memorySearchLimit ?? 5,
            skillConfidenceThreshold: config.skillConfidenceThreshold ?? 0.6,
            tenantId: config.tenantId ?? 'default',
        };
        this.memoryManager = new MemoryManager_js_1.MemoryManager({
            dimension: 384,
            maxEntries: 100000,
        });
        this.skillExecutor = new SkillExecutor_js_1.SkillExecutor({
            enableBuiltinSkills: this.config.enableSkills,
            memoryManager: this.memoryManager,
            tenantId: this.config.tenantId,
        });
    }
    /**
     * Set the LLM provider for enhanced responses
     */
    setLLMProvider(provider) {
        this.llmProvider = provider;
    }
    /**
     * Process a chat message with full enhancement
     */
    async processMessage(message, context) {
        const startTime = Date.now();
        const response = {
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
        let detectedSkills = [];
        if (this.config.enableSkills) {
            detectedSkills = this.skillExecutor.detectSkills(message);
            response.metadata.skillsDetected = detectedSkills.length;
        }
        // Step 2: Search memory for context
        let relevantMemories = [];
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
                response.metadata.memorySearched = true;
            }
            catch (error) {
                console.warn('Memory search failed:', error);
            }
        }
        // Step 3: Execute high-confidence skills
        const skillResponses = [];
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
                        .filter((s) => s.type === 'message' && !!s.content)
                        .map((s) => s.content);
                    if (messages.length > 0) {
                        skillResponses.push(messages.join('\n'));
                    }
                    response.skillsUsed.push({
                        skillId: match.skill.id,
                        skillName: match.skill.name,
                        success: result.success,
                        output: result.output,
                    });
                    // Count stored memories
                    if (result.memoriesToStore) {
                        response.memoriesStored += result.memoriesToStore.length;
                    }
                }
                catch (error) {
                    console.warn(`Skill ${match.skill.id} execution failed:`, error);
                    response.skillsUsed.push({
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
        response.metadata.processingTime = Date.now() - startTime;
        return response;
    }
    /**
     * Store a memory from the conversation
     */
    async storeMemory(content, tenantId, options) {
        const entry = await this.memoryManager.store(tenantId, `memory-${Date.now()}`, content, {
            sessionId: options?.sessionId,
            type: options?.type || 'semantic',
            text: content,
            tags: options?.tags || [],
        });
        return entry.id;
    }
    /**
     * Get available skills
     */
    getAvailableSkills() {
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
    getMemoryStats() {
        return this.memoryManager.stats();
    }
    /**
     * Generate proactive assistance hints
     */
    generateProactiveHints(message, detectedSkills) {
        const hints = [];
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
exports.ChatEnhancer = ChatEnhancer;
/**
 * Factory function to create ChatEnhancer
 */
function createChatEnhancer(config) {
    return new ChatEnhancer(config);
}
exports.default = ChatEnhancer;
//# sourceMappingURL=ChatEnhancer.js.map