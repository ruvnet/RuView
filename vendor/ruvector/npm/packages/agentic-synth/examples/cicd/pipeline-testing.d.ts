/**
 * CI/CD Pipeline Testing Examples
 *
 * This module demonstrates how to use agentic-synth for comprehensive
 * pipeline testing including:
 * - Dynamic test case generation
 * - Edge case scenario creation
 * - Performance test data at scale
 * - Security testing datasets
 * - Multi-stage pipeline data flows
 *
 * @module pipeline-testing
 */
import { GenerationResult } from '../../src/index.js';
/**
 * Pipeline testing configuration
 */
export interface PipelineTestConfig {
    provider?: 'gemini' | 'openrouter';
    apiKey?: string;
    outputDir?: string;
    seed?: string | number;
    parallel?: boolean;
    concurrency?: number;
}
/**
 * Test case metadata
 */
export interface TestCase {
    id: string;
    name: string;
    description: string;
    category: string;
    priority: 'critical' | 'high' | 'medium' | 'low';
    data: any;
    expectedResult?: any;
    assertions?: string[];
}
/**
 * Pipeline testing orchestrator
 */
export declare class PipelineTester {
    private synth;
    private config;
    constructor(config?: PipelineTestConfig);
    /**
     * Generate dynamic test cases based on specifications
     *
     * Creates comprehensive test cases from high-level requirements,
     * including positive, negative, and edge cases.
     */
    generateDynamicTestCases(options: {
        feature: string;
        scenarios?: string[];
        count?: number;
        includeBoundary?: boolean;
        includeNegative?: boolean;
    }): Promise<GenerationResult<TestCase>>;
    /**
     * Generate edge case scenarios
     *
     * Creates extreme and boundary condition test data to catch
     * potential bugs and edge cases.
     */
    generateEdgeCases(options: {
        dataType: string;
        count?: number;
        extremes?: boolean;
    }): Promise<GenerationResult>;
    /**
     * Generate performance test data at scale
     *
     * Creates large-scale datasets for performance and stress testing
     * with realistic data distributions.
     */
    generatePerformanceTestData(options: {
        scenario: string;
        dataPoints?: number;
        concurrent?: boolean;
        timeRange?: {
            start: Date;
            end: Date;
        };
    }): Promise<GenerationResult>;
    /**
     * Generate security testing datasets
     *
     * Creates security-focused test data including:
     * - SQL injection payloads
     * - XSS attack vectors
     * - Authentication bypass attempts
     * - CSRF tokens and scenarios
     * - Rate limiting tests
     */
    generateSecurityTestData(options?: {
        attackVectors?: string[];
        count?: number;
    }): Promise<GenerationResult>;
    /**
     * Generate multi-stage pipeline test data
     *
     * Creates interconnected test data that flows through
     * multiple pipeline stages (build, test, deploy).
     */
    generatePipelineData(options?: {
        stages?: string[];
        jobsPerStage?: number;
    }): Promise<Record<string, GenerationResult>>;
    /**
     * Generate regression test data
     *
     * Creates test data specifically for regression testing,
     * including historical bug scenarios and known issues.
     */
    generateRegressionTests(options?: {
        bugCount?: number;
        includeFixed?: boolean;
    }): Promise<GenerationResult>;
    /**
     * Generate comprehensive test suite
     *
     * Combines all test data generation methods into a complete
     * test suite for CI/CD pipelines.
     */
    generateComprehensiveTestSuite(options?: {
        feature: string;
        testCases?: number;
        edgeCases?: number;
        performanceTests?: number;
        securityTests?: number;
    }): Promise<void>;
    /**
     * Save result to file
     */
    private saveResult;
}
/**
 * Example: GitHub Actions Integration
 */
declare function githubActionsPipelineTest(): Promise<void>;
/**
 * Example: GitLab CI Integration
 */
declare function gitlabCIPipelineTest(): Promise<void>;
/**
 * Example: Jenkins Pipeline Integration
 */
declare function jenkinsPipelineTest(): Promise<void>;
export { githubActionsPipelineTest, gitlabCIPipelineTest, jenkinsPipelineTest };
//# sourceMappingURL=pipeline-testing.d.ts.map