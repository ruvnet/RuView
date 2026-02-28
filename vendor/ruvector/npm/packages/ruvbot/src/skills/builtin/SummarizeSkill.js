"use strict";
/**
 * Summarize Skill - Summarize text, documents, and conversations
 *
 * Provides:
 * - Text summarization
 * - Key point extraction
 * - Conversation summary
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SummarizeSkills = exports.ConversationSummarySkill = exports.SummarizeSkill = void 0;
exports.SummarizeSkill = {
    id: 'summarize',
    name: 'Summarize',
    description: 'Summarize text or extract key points',
    version: '1.0.0',
    triggers: [
        { type: 'keyword', value: 'summarize', confidence: 0.95 },
        { type: 'keyword', value: 'summary', confidence: 0.9 },
        { type: 'keyword', value: 'tldr', confidence: 0.95 },
        { type: 'keyword', value: 'key points', confidence: 0.85 },
        { type: 'keyword', value: 'main ideas', confidence: 0.8 },
        { type: 'keyword', value: 'condense', confidence: 0.7 },
        { type: 'intent', value: 'summarize', confidence: 0.95 },
    ],
    parameters: {
        type: 'object',
        properties: {
            text: {
                type: 'string',
                description: 'The text to summarize',
            },
            style: {
                type: 'string',
                description: 'Summary style: brief, bullet, detailed',
                default: 'bullet',
            },
            maxLength: {
                type: 'number',
                description: 'Maximum length in words',
                default: 100,
            },
        },
        required: ['text'],
    },
    execute: summarizeExecutor,
};
exports.ConversationSummarySkill = {
    id: 'conversation-summary',
    name: 'Conversation Summary',
    description: 'Summarize the current conversation',
    version: '1.0.0',
    triggers: [
        { type: 'keyword', value: 'summarize conversation', confidence: 0.95 },
        { type: 'keyword', value: 'summarize our chat', confidence: 0.9 },
        { type: 'keyword', value: 'what did we discuss', confidence: 0.85 },
        { type: 'keyword', value: 'recap', confidence: 0.8 },
        { type: 'intent', value: 'conversation_summary', confidence: 0.95 },
    ],
    parameters: {
        type: 'object',
        properties: {
            includeActions: {
                type: 'boolean',
                description: 'Include action items',
                default: true,
            },
        },
        required: [],
    },
    execute: conversationSummaryExecutor,
};
function extractKeyPhrases(text) {
    const phrases = [];
    // Extract sentences that might be important (contain key indicators)
    const sentences = text.split(/[.!?]+/).filter((s) => s.trim().length > 20);
    for (const sentence of sentences) {
        const trimmed = sentence.trim();
        const lower = trimmed.toLowerCase();
        // Check for importance indicators
        const hasKeyword = lower.includes('important') ||
            lower.includes('key') ||
            lower.includes('main') ||
            lower.includes('critical') ||
            lower.includes('essential') ||
            lower.includes('must') ||
            lower.includes('should') ||
            lower.includes('conclusion') ||
            lower.includes('result') ||
            lower.includes('therefore') ||
            lower.includes('however') ||
            lower.includes('first') ||
            lower.includes('finally');
        if (hasKeyword && phrases.length < 5) {
            phrases.push(trimmed);
        }
    }
    // If no key phrases found, take first few sentences
    if (phrases.length === 0 && sentences.length > 0) {
        return sentences.slice(0, 3).map((s) => s.trim());
    }
    return phrases;
}
function getTextStats(text) {
    const words = text.split(/\s+/).filter((w) => w.length > 0).length;
    const sentences = text.split(/[.!?]+/).filter((s) => s.trim().length > 0).length;
    const paragraphs = text.split(/\n\n+/).filter((p) => p.trim().length > 0).length;
    // Average reading speed: 200-250 words per minute
    const minutes = Math.ceil(words / 225);
    const readingTime = minutes < 1 ? 'less than 1 minute' : `about ${minutes} minute${minutes > 1 ? 's' : ''}`;
    return { words, sentences, paragraphs, readingTime };
}
async function* summarizeExecutor(context, params) {
    const text = params.text;
    const style = params.style || 'bullet';
    const maxLength = params.maxLength || 100;
    yield {
        type: 'message',
        content: 'Analyzing text for summarization...',
    };
    yield {
        type: 'progress',
        progress: 30,
    };
    const stats = getTextStats(text);
    const keyPhrases = extractKeyPhrases(text);
    yield {
        type: 'progress',
        progress: 70,
    };
    // Build summary report
    let summaryIntro = `**Text Statistics**\n`;
    summaryIntro += `- Words: ${stats.words}\n`;
    summaryIntro += `- Sentences: ${stats.sentences}\n`;
    summaryIntro += `- Reading time: ${stats.readingTime}\n\n`;
    if (keyPhrases.length > 0) {
        summaryIntro += `**Key Points Detected**:\n`;
        for (const phrase of keyPhrases) {
            summaryIntro += `- ${phrase.substring(0, 150)}${phrase.length > 150 ? '...' : ''}\n`;
        }
        summaryIntro += '\n';
    }
    summaryIntro += `*The LLM will now generate a ${style} summary (max ${maxLength} words).*`;
    yield {
        type: 'progress',
        progress: 100,
    };
    yield {
        type: 'message',
        content: summaryIntro,
    };
    return {
        success: true,
        output: {
            stats,
            keyPhrases,
            style,
            maxLength,
            originalLength: text.length,
        },
        message: `Prepared summary for text (${stats.words} words)`,
        memoriesToStore: [
            {
                content: `Summarized text: ${keyPhrases.join('. ').substring(0, 200)}`,
                type: 'semantic',
            },
        ],
    };
}
async function* conversationSummaryExecutor(context, params) {
    const includeActions = params.includeActions !== false;
    const history = context.conversationHistory || [];
    yield {
        type: 'message',
        content: 'Analyzing conversation...',
    };
    yield {
        type: 'progress',
        progress: 40,
    };
    // Analyze conversation
    const userMessages = history.filter((m) => m.role === 'user');
    const assistantMessages = history.filter((m) => m.role === 'assistant');
    const topics = [];
    const potentialActions = [];
    // Extract topics and action items
    for (const msg of userMessages) {
        const content = msg.content.toLowerCase();
        // Look for question patterns
        if (content.includes('?')) {
            const question = msg.content.split('?')[0] + '?';
            if (question.length < 100) {
                topics.push(`Question: ${question}`);
            }
        }
        // Look for action requests
        if (content.includes('please') || content.includes('can you') || content.includes('help')) {
            potentialActions.push(msg.content.substring(0, 80));
        }
    }
    yield {
        type: 'progress',
        progress: 80,
    };
    let summary = `**Conversation Summary**\n\n`;
    summary += `- **Messages**: ${history.length} total (${userMessages.length} from you, ${assistantMessages.length} from me)\n`;
    if (topics.length > 0) {
        summary += `\n**Topics Discussed**:\n`;
        for (const topic of topics.slice(0, 5)) {
            summary += `- ${topic}\n`;
        }
    }
    if (includeActions && potentialActions.length > 0) {
        summary += `\n**Potential Action Items**:\n`;
        for (const action of potentialActions.slice(0, 3)) {
            summary += `- ${action}\n`;
        }
    }
    yield {
        type: 'progress',
        progress: 100,
    };
    yield {
        type: 'message',
        content: summary,
    };
    return {
        success: true,
        output: {
            messageCount: history.length,
            userMessages: userMessages.length,
            assistantMessages: assistantMessages.length,
            topics,
            potentialActions: includeActions ? potentialActions : [],
        },
        message: `Summarized conversation with ${history.length} messages`,
    };
}
exports.SummarizeSkills = [exports.SummarizeSkill, exports.ConversationSummarySkill];
exports.default = exports.SummarizeSkills;
//# sourceMappingURL=SummarizeSkill.js.map