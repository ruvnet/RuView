/**
 * Enhanced Psycho-Symbolic Reasoning with Learning Integration
 * Fixes novel knowledge integration and adds cross-tool learning
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
export declare class LearningPsychoSymbolicTools {
    private knowledgeBase;
    private learningCoordinator;
    private performanceCache;
    private reasoningCache;
    constructor();
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    private performLearningReasoning;
    private identifyCognitivePatterns;
    private extractEntitiesAndConcepts;
    private enhancedKnowledgeTraversal;
    private generateCreativeAssociations;
    private generateLearningDomainInsights;
    private synthesizeLearningAnswer;
    private enhancedKnowledgeQuery;
    private getLearningStatus;
}
