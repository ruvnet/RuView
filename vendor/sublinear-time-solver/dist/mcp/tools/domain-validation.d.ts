/**
 * Domain Validation MCP Tools
 * Provides comprehensive validation, testing, and analysis for domains
 */
import { Tool } from '@modelcontextprotocol/sdk/types.js';
import { DomainRegistry } from './domain-registry.js';
export declare class DomainValidationTools {
    private domainRegistry;
    constructor(domainRegistry: DomainRegistry);
    getTools(): Tool[];
    handleToolCall(name: string, args: any): Promise<any>;
    private validateDomain;
    private testDomain;
    private analyzeConflicts;
    private suggestImprovements;
    private testDomainDetection;
    private benchmarkDomains;
    private validateSchema;
    private validateSemantics;
    private checkDomainConflicts;
    private validateDependencies;
    private validatePerformance;
    private runIndividualTest;
    private getTestRecommendation;
    private analyzeSpecificConflict;
    private analyzeImprovementArea;
    private compareWithSimilarDomains;
    private testSingleQueryDetection;
    private runDomainBenchmark;
}
