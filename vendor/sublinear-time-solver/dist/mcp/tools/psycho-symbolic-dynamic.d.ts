/**
 * Enhanced Psycho-Symbolic Tools with Dynamic Domain Support
 * Extends existing functionality while preserving all current capabilities
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { PsychoSymbolicTools } from './psycho-symbolic.js';
import { DomainRegistry } from './domain-registry.js';
export declare class DynamicPsychoSymbolicTools extends PsychoSymbolicTools {
    private domainRegistry;
    constructor(domainRegistry?: DomainRegistry);
    private initializeDynamicDomainIntegration;
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    private performEnhancedReasoning;
    private testDomainDetection;
    private advancedKnowledgeQueryDynamic;
    private buildDomainFilters;
    private performEnhancedDomainDetection;
    private updateDomainEngine;
    private getDynamicDomainsCount;
    private getBuiltinDomainsCount;
    private updateDynamicDomainUsage;
    private testDomainDetectionSingle;
    private applyDomainWeighting;
    getDomainRegistry(): DomainRegistry;
}
