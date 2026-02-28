/**
 * CI/CD Test Data Generator Examples
 *
 * This module demonstrates how to use agentic-synth to generate
 * comprehensive test data for CI/CD pipelines including:
 * - Database fixtures for integration tests
 * - API mock responses
 * - User session data for E2E tests
 * - Load testing datasets
 * - Configuration variations for multi-environment testing
 *
 * @module test-data-generator
 */
import { GenerationResult } from '../../src/index.js';
/**
 * Configuration for test data generation
 */
export interface TestDataConfig {
    outputDir: string;
    format: 'json' | 'csv' | 'array';
    provider?: 'gemini' | 'openrouter';
    apiKey?: string;
    seed?: string | number;
}
/**
 * Test data generator class for CI/CD pipelines
 */
export declare class CICDTestDataGenerator {
    private synth;
    private config;
    constructor(config?: Partial<TestDataConfig>);
    /**
     * Generate database fixtures for integration tests
     *
     * Creates realistic database records with proper relationships
     * and constraints for testing database operations.
     *
     * @example
     * ```typescript
     * const generator = new CICDTestDataGenerator();
     * const fixtures = await generator.generateDatabaseFixtures({
     *   users: 50,
     *   posts: 200,
     *   comments: 500
     * });
     * ```
     */
    generateDatabaseFixtures(options?: {
        users?: number;
        posts?: number;
        comments?: number;
        orders?: number;
        products?: number;
    }): Promise<Record<string, GenerationResult>>;
    /**
     * Generate API mock responses for testing
     *
     * Creates realistic API responses with various status codes,
     * headers, and payloads for comprehensive API testing.
     */
    generateAPIMockResponses(options?: {
        endpoints?: string[];
        responsesPerEndpoint?: number;
        includeErrors?: boolean;
    }): Promise<GenerationResult>;
    /**
     * Generate user session data for E2E tests
     *
     * Creates realistic user sessions with cookies, tokens,
     * and session state for end-to-end testing.
     */
    generateUserSessions(options?: {
        sessionCount?: number;
        includeAnonymous?: boolean;
    }): Promise<GenerationResult>;
    /**
     * Generate load testing datasets
     *
     * Creates large-scale datasets for load and performance testing
     * with configurable data patterns and distributions.
     */
    generateLoadTestData(options?: {
        requestCount?: number;
        concurrent?: number;
        duration?: number;
    }): Promise<GenerationResult>;
    /**
     * Generate configuration variations for multi-environment testing
     *
     * Creates configuration files for different environments
     * (dev, staging, production) with realistic values.
     */
    generateEnvironmentConfigs(options?: {
        environments?: string[];
        includeSecrets?: boolean;
    }): Promise<Record<string, GenerationResult>>;
    /**
     * Generate all test data at once
     *
     * Convenience method to generate all types of test data
     * in a single operation.
     */
    generateAll(options?: {
        users?: number;
        posts?: number;
        comments?: number;
        orders?: number;
        products?: number;
        apiMocks?: number;
        sessions?: number;
        loadTestRequests?: number;
    }): Promise<void>;
    /**
     * Save generation result to file
     */
    private saveToFile;
}
/**
 * Example usage in CI/CD pipeline
 */
declare function cicdExample(): Promise<void>;
/**
 * GitHub Actions example
 */
declare function githubActionsExample(): Promise<void>;
/**
 * GitLab CI example
 */
declare function gitlabCIExample(): Promise<void>;
export { cicdExample, githubActionsExample, gitlabCIExample };
//# sourceMappingURL=test-data-generator.d.ts.map