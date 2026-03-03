/**
 * Enhanced Psycho-Symbolic Reasoning MCP Tools
 * Full implementation with domain-agnostic reasoning and fallback mechanisms
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
export declare class PsychoSymbolicTools {
    private knowledgeBase;
    private reasoningCache;
    constructor();
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    private performDeepReasoning;
    private generateDomainInsights;
    private applyContextualReasoning;
    private analyzeEdgeCases;
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
    private generateDefaultInsights;
    private queryKnowledgeGraph;
    private addKnowledge;
}
export default PsychoSymbolicTools;
