/**
 * Enhanced Psycho-Symbolic Reasoning MCP Tools
 * Full implementation with real reasoning, knowledge graph, and inference engine
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
export declare class EnhancedPsychoSymbolicTools {
    private knowledgeBase;
    private reasoningCache;
    constructor();
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    private performDeepReasoning;
    private identifyCognitivePatterns;
    private extractEntitiesAndConcepts;
    private extractLogicalComponents;
    private traverseKnowledgeGraph;
    private buildInferenceChain;
    private findTransitiveChains;
    private generateHypotheses;
    private detectContradictions;
    private resolveContradictions;
    private synthesizeCompleteAnswer;
    private queryKnowledgeGraph;
    private addKnowledge;
}
export default EnhancedPsychoSymbolicTools;
