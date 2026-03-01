"use strict";
/**
 * Web Search Skill - Search the web for information
 *
 * Uses DuckDuckGo Instant Answer API for web search
 * Integrates with RuVector for result embedding and memory storage
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.WebSearchSkill = void 0;
exports.WebSearchSkill = {
    id: 'web-search',
    name: 'Web Search',
    description: 'Search the web for information using DuckDuckGo',
    version: '1.0.0',
    triggers: [
        { type: 'keyword', value: 'search', confidence: 0.9 },
        { type: 'keyword', value: 'find', confidence: 0.7 },
        { type: 'keyword', value: 'lookup', confidence: 0.7 },
        { type: 'keyword', value: 'what is', confidence: 0.6 },
        { type: 'keyword', value: 'who is', confidence: 0.6 },
        { type: 'keyword', value: 'how to', confidence: 0.5 },
        { type: 'intent', value: 'web_search', confidence: 0.95 },
    ],
    parameters: {
        type: 'object',
        properties: {
            query: {
                type: 'string',
                description: 'The search query',
            },
            maxResults: {
                type: 'number',
                description: 'Maximum number of results to return',
                default: 5,
            },
        },
        required: ['query'],
    },
    execute: webSearchExecutor,
};
async function* webSearchExecutor(context, params) {
    const query = params.query;
    const maxResults = params.maxResults ?? 5;
    yield {
        type: 'message',
        content: `Searching the web for: "${query}"...`,
    };
    yield {
        type: 'progress',
        progress: 20,
    };
    try {
        // Use DuckDuckGo Instant Answer API (no API key required)
        const searchUrl = `https://api.duckduckgo.com/?q=${encodeURIComponent(query)}&format=json&no_html=1&skip_disambig=1`;
        const response = await fetch(searchUrl, {
            headers: {
                'User-Agent': 'RuvBot/1.0 (AI Assistant)',
            },
        });
        if (!response.ok) {
            throw new Error(`Search failed: ${response.statusText}`);
        }
        yield {
            type: 'progress',
            progress: 60,
        };
        const data = (await response.json());
        // Extract relevant information
        const results = [];
        // Check for direct answer
        if (data.Answer) {
            results.push({
                title: 'Direct Answer',
                url: '',
                snippet: data.Answer,
            });
        }
        // Check for abstract/definition
        if (data.AbstractText) {
            results.push({
                title: data.Heading || 'Summary',
                url: data.AbstractURL || '',
                snippet: data.AbstractText,
            });
        }
        if (data.Definition) {
            results.push({
                title: 'Definition',
                url: data.DefinitionURL || '',
                snippet: data.Definition,
            });
        }
        // Add related topics
        if (data.RelatedTopics) {
            for (const topic of data.RelatedTopics.slice(0, maxResults - results.length)) {
                if (topic.Text && topic.FirstURL) {
                    results.push({
                        title: topic.Text.split(' - ')[0] || 'Related',
                        url: topic.FirstURL,
                        snippet: topic.Text,
                    });
                }
            }
        }
        // Add results
        if (data.Results) {
            for (const result of data.Results.slice(0, maxResults - results.length)) {
                if (result.Text && result.FirstURL) {
                    results.push({
                        title: result.Text.split(' - ')[0] || 'Result',
                        url: result.FirstURL,
                        snippet: result.Text,
                    });
                }
            }
        }
        yield {
            type: 'progress',
            progress: 100,
        };
        // Format results for output
        let formattedResults = '';
        if (results.length === 0) {
            formattedResults = `No direct results found for "${query}". Try a different search query.`;
        }
        else {
            formattedResults = results
                .map((r, i) => {
                let entry = `${i + 1}. **${r.title}**\n   ${r.snippet}`;
                if (r.url) {
                    entry += `\n   [Learn more](${r.url})`;
                }
                return entry;
            })
                .join('\n\n');
        }
        yield {
            type: 'message',
            content: formattedResults,
        };
        return {
            success: true,
            output: {
                query,
                results,
                resultCount: results.length,
            },
            message: `Found ${results.length} results for "${query}"`,
            memoriesToStore: results.length > 0
                ? [
                    {
                        content: `Web search for "${query}": ${results.map(r => r.snippet).join(' | ')}`,
                        type: 'semantic',
                    },
                ]
                : undefined,
        };
    }
    catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        yield {
            type: 'message',
            content: `Search failed: ${errorMessage}. Please try again.`,
        };
        return {
            success: false,
            output: { error: errorMessage },
            message: `Web search failed: ${errorMessage}`,
        };
    }
}
exports.default = exports.WebSearchSkill;
//# sourceMappingURL=WebSearchSkill.js.map