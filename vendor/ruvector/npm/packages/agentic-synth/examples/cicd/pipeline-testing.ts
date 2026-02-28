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

import { AgenticSynth, createSynth, GenerationResult, SynthError } from '../../src/index.js';
import * as fs from 'fs/promises';
import * as path from 'path';

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
export class PipelineTester {
  private synth: AgenticSynth;
  private config: PipelineTestConfig;

  constructor(config: PipelineTestConfig = {}) {
    this.config = {
      provider: config.provider || 'gemini',
      apiKey: config.apiKey || process.env.GEMINI_API_KEY,
      outputDir: config.outputDir || './pipeline-tests',
      seed: config.seed || Date.now(),
      parallel: config.parallel !== false,
      concurrency: config.concurrency || 5
    };

    this.synth = createSynth({
      provider: this.config.provider,
      apiKey: this.config.apiKey,
      cacheStrategy: 'memory',
      maxRetries: 3
    });
  }

  /**
   * Generate dynamic test cases based on specifications
   *
   * Creates comprehensive test cases from high-level requirements,
   * including positive, negative, and edge cases.
   */
  async generateDynamicTestCases(options: {
    feature: string;
    scenarios?: string[];
    count?: number;
    includeBoundary?: boolean;
    includeNegative?: boolean;
  }): Promise<GenerationResult<TestCase>> {
    const {
      feature,
      scenarios = ['happy_path', 'error_handling', 'edge_cases'],
      count = 20,
      includeBoundary = true,
      includeNegative = true
    } = options;

    console.log(`Generating test cases for feature: ${feature}...`);

    try {
      const testCaseSchema = {
        id: { type: 'uuid', required: true },
        name: { type: 'string', required: true },
        description: { type: 'text', required: true },
        category: {
          type: 'enum',
          values: ['unit', 'integration', 'e2e', 'performance', 'security'],
          required: true
        },
        scenario: {
          type: 'enum',
          values: scenarios,
          required: true
        },
        priority: {
          type: 'enum',
          values: ['critical', 'high', 'medium', 'low'],
          required: true
        },
        testType: {
          type: 'enum',
          values: ['positive', 'negative', 'boundary', 'edge'],
          required: true
        },
        input: { type: 'object', required: true },
        expectedOutput: { type: 'object', required: true },
        preconditions: { type: 'array', items: { type: 'string' } },
        steps: { type: 'array', items: { type: 'string' } },
        assertions: { type: 'array', items: { type: 'string' } },
        tags: { type: 'array', items: { type: 'string' } },
        timeout: { type: 'integer', min: 1000, max: 60000, required: true },
        retryable: { type: 'boolean', required: true },
        flaky: { type: 'boolean', required: true },
        metadata: {
          type: 'object',
          properties: {
            author: { type: 'string' },
            createdAt: { type: 'timestamp' },
            jiraTicket: { type: 'string' },
            relatedTests: { type: 'array', items: { type: 'string' } }
          }
        }
      };

      const result = await this.synth.generateStructured({
        count,
        schema: testCaseSchema,
        seed: this.config.seed,
        constraints: {
          feature,
          includeBoundary,
          includeNegative
        }
      });

      await this.saveResult('test-cases', result);

      console.log('✅ Test cases generated successfully');
      console.log(`   Total cases: ${result.metadata.count}`);
      console.log(`   Duration: ${result.metadata.duration}ms`);

      return result as GenerationResult<TestCase>;
    } catch (error) {
      console.error('❌ Failed to generate test cases:', error);
      throw new SynthError('Test case generation failed', 'TEST_CASE_ERROR', error);
    }
  }

  /**
   * Generate edge case scenarios
   *
   * Creates extreme and boundary condition test data to catch
   * potential bugs and edge cases.
   */
  async generateEdgeCases(options: {
    dataType: string;
    count?: number;
    extremes?: boolean;
  }): Promise<GenerationResult> {
    const {
      dataType,
      count = 30,
      extremes = true
    } = options;

    console.log(`Generating edge cases for ${dataType}...`);

    try {
      // Define schemas for different edge case types
      const edgeCaseSchemas: Record<string, any> = {
        string: {
          type: 'string',
          variants: [
            'empty',
            'very_long',
            'special_characters',
            'unicode',
            'sql_injection',
            'xss_payload',
            'null_bytes',
            'whitespace_only'
          ]
        },
        number: {
          type: 'number',
          variants: [
            'zero',
            'negative',
            'very_large',
            'very_small',
            'float_precision',
            'infinity',
            'nan',
            'negative_zero'
          ]
        },
        array: {
          type: 'array',
          variants: [
            'empty',
            'single_element',
            'very_large',
            'nested_deeply',
            'mixed_types',
            'circular_reference'
          ]
        },
        object: {
          type: 'object',
          variants: [
            'empty',
            'null_values',
            'undefined_values',
            'nested_deeply',
            'large_keys',
            'special_key_names'
          ]
        }
      };

      const schema = {
        id: { type: 'uuid', required: true },
        edgeCase: { type: 'string', required: true },
        variant: { type: 'string', required: true },
        value: { type: 'any', required: true },
        description: { type: 'text', required: true },
        expectedBehavior: { type: 'string', required: true },
        category: {
          type: 'enum',
          values: ['boundary', 'extreme', 'invalid', 'malformed', 'security'],
          required: true
        },
        severity: {
          type: 'enum',
          values: ['critical', 'high', 'medium', 'low'],
          required: true
        },
        testData: { type: 'object', required: true }
      };

      const result = await this.synth.generateStructured({
        count,
        schema,
        seed: this.config.seed,
        constraints: {
          dataType,
          extremes,
          variants: edgeCaseSchemas[dataType]?.variants || []
        }
      });

      await this.saveResult('edge-cases', result);

      console.log('✅ Edge cases generated successfully');
      console.log(`   Total cases: ${result.metadata.count}`);

      return result;
    } catch (error) {
      console.error('❌ Failed to generate edge cases:', error);
      throw new SynthError('Edge case generation failed', 'EDGE_CASE_ERROR', error);
    }
  }

  /**
   * Generate performance test data at scale
   *
   * Creates large-scale datasets for performance and stress testing
   * with realistic data distributions.
   */
  async generatePerformanceTestData(options: {
    scenario: string;
    dataPoints?: number;
    concurrent?: boolean;
    timeRange?: { start: Date; end: Date };
  }): Promise<GenerationResult> {
    const {
      scenario,
      dataPoints = 100000,
      concurrent = true,
      timeRange = {
        start: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
        end: new Date()
      }
    } = options;

    console.log(`Generating performance test data for ${scenario}...`);

    try {
      // Generate time-series data for realistic performance testing
      const result = await this.synth.generateTimeSeries({
        count: dataPoints,
        startDate: timeRange.start,
        endDate: timeRange.end,
        interval: '1m',
        metrics: ['requests', 'latency', 'errors', 'cpu', 'memory'],
        trend: 'random',
        seasonality: true,
        noise: 0.2
      });

      await this.saveResult(`performance-${scenario}`, result);

      console.log('✅ Performance test data generated successfully');
      console.log(`   Data points: ${result.metadata.count}`);
      console.log(`   Duration: ${result.metadata.duration}ms`);

      return result;
    } catch (error) {
      console.error('❌ Failed to generate performance test data:', error);
      throw new SynthError('Performance data generation failed', 'PERF_DATA_ERROR', error);
    }
  }

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
  async generateSecurityTestData(options: {
    attackVectors?: string[];
    count?: number;
  } = {}): Promise<GenerationResult> {
    const {
      attackVectors = ['sql_injection', 'xss', 'csrf', 'auth_bypass', 'path_traversal'],
      count = 50
    } = options;

    console.log('Generating security test data...');

    try {
      const securityTestSchema = {
        id: { type: 'uuid', required: true },
        attackType: {
          type: 'enum',
          values: attackVectors,
          required: true
        },
        severity: {
          type: 'enum',
          values: ['critical', 'high', 'medium', 'low'],
          required: true
        },
        payload: { type: 'string', required: true },
        description: { type: 'text', required: true },
        targetEndpoint: { type: 'string', required: true },
        method: { type: 'enum', values: ['GET', 'POST', 'PUT', 'DELETE'], required: true },
        headers: {
          type: 'object',
          properties: {
            'Content-Type': { type: 'string' },
            'Authorization': { type: 'string' },
            'X-CSRF-Token': { type: 'string' }
          }
        },
        expectedResponse: {
          type: 'object',
          properties: {
            statusCode: { type: 'integer' },
            blocked: { type: 'boolean' },
            sanitized: { type: 'boolean' }
          }
        },
        mitigation: { type: 'string', required: true },
        cvssScore: { type: 'decimal', min: 0, max: 10, required: false },
        references: { type: 'array', items: { type: 'url' } }
      };

      const result = await this.synth.generateStructured({
        count,
        schema: securityTestSchema,
        seed: this.config.seed
      });

      await this.saveResult('security-tests', result);

      console.log('✅ Security test data generated successfully');
      console.log(`   Test cases: ${result.metadata.count}`);
      console.log(`   Attack vectors: ${attackVectors.join(', ')}`);

      return result;
    } catch (error) {
      console.error('❌ Failed to generate security test data:', error);
      throw new SynthError('Security test generation failed', 'SECURITY_TEST_ERROR', error);
    }
  }

  /**
   * Generate multi-stage pipeline test data
   *
   * Creates interconnected test data that flows through
   * multiple pipeline stages (build, test, deploy).
   */
  async generatePipelineData(options: {
    stages?: string[];
    jobsPerStage?: number;
  } = {}): Promise<Record<string, GenerationResult>> {
    const {
      stages = ['build', 'test', 'deploy'],
      jobsPerStage = 10
    } = options;

    console.log('Generating multi-stage pipeline data...');

    try {
      const results: Record<string, GenerationResult> = {};

      for (const stage of stages) {
        const stageSchema = {
          id: { type: 'uuid', required: true },
          stage: { type: 'string', required: true, default: stage },
          jobName: { type: 'string', required: true },
          status: {
            type: 'enum',
            values: ['pending', 'running', 'success', 'failed', 'cancelled', 'skipped'],
            required: true
          },
          startedAt: { type: 'timestamp', required: true },
          completedAt: { type: 'timestamp', required: false },
          duration: { type: 'integer', min: 0, required: false },
          exitCode: { type: 'integer', required: false },
          logs: { type: 'text', required: false },
          artifacts: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                name: { type: 'string' },
                path: { type: 'string' },
                size: { type: 'integer' }
              }
            }
          },
          dependencies: { type: 'array', items: { type: 'string' } },
          environment: {
            type: 'object',
            properties: {
              name: { type: 'string' },
              variables: { type: 'object' }
            }
          },
          metrics: {
            type: 'object',
            properties: {
              cpuUsage: { type: 'decimal' },
              memoryUsage: { type: 'decimal' },
              diskIO: { type: 'integer' }
            }
          }
        };

        const result = await this.synth.generateStructured({
          count: jobsPerStage,
          schema: stageSchema,
          seed: `${this.config.seed}-${stage}`
        });

        results[stage] = result;
        await this.saveResult(`pipeline-${stage}`, result);
      }

      console.log('✅ Pipeline data generated successfully');
      console.log(`   Stages: ${stages.join(' → ')}`);
      console.log(`   Jobs per stage: ${jobsPerStage}`);

      return results;
    } catch (error) {
      console.error('❌ Failed to generate pipeline data:', error);
      throw new SynthError('Pipeline data generation failed', 'PIPELINE_ERROR', error);
    }
  }

  /**
   * Generate regression test data
   *
   * Creates test data specifically for regression testing,
   * including historical bug scenarios and known issues.
   */
  async generateRegressionTests(options: {
    bugCount?: number;
    includeFixed?: boolean;
  } = {}): Promise<GenerationResult> {
    const {
      bugCount = 25,
      includeFixed = true
    } = options;

    console.log('Generating regression test data...');

    try {
      const regressionSchema = {
        id: { type: 'uuid', required: true },
        bugId: { type: 'string', required: true },
        title: { type: 'string', required: true },
        description: { type: 'text', required: true },
        severity: {
          type: 'enum',
          values: ['critical', 'high', 'medium', 'low'],
          required: true
        },
        status: {
          type: 'enum',
          values: ['open', 'fixed', 'verified', 'wont_fix'],
          required: true
        },
        reproducibleSteps: { type: 'array', items: { type: 'string' } },
        testData: { type: 'object', required: true },
        expectedBehavior: { type: 'text', required: true },
        actualBehavior: { type: 'text', required: true },
        fixedInVersion: { type: 'string', required: false },
        relatedBugs: { type: 'array', items: { type: 'string' } },
        affectedVersions: { type: 'array', items: { type: 'string' } },
        testCoverage: {
          type: 'object',
          properties: {
            unitTest: { type: 'boolean' },
            integrationTest: { type: 'boolean' },
            e2eTest: { type: 'boolean' }
          }
        }
      };

      const result = await this.synth.generateStructured({
        count: bugCount,
        schema: regressionSchema,
        seed: this.config.seed,
        constraints: { includeFixed }
      });

      await this.saveResult('regression-tests', result);

      console.log('✅ Regression test data generated successfully');
      console.log(`   Bug scenarios: ${result.metadata.count}`);

      return result;
    } catch (error) {
      console.error('❌ Failed to generate regression test data:', error);
      throw new SynthError('Regression test generation failed', 'REGRESSION_ERROR', error);
    }
  }

  /**
   * Generate comprehensive test suite
   *
   * Combines all test data generation methods into a complete
   * test suite for CI/CD pipelines.
   */
  async generateComprehensiveTestSuite(options: {
    feature: string;
    testCases?: number;
    edgeCases?: number;
    performanceTests?: number;
    securityTests?: number;
  } = { feature: 'default' }): Promise<void> {
    console.log('🚀 Generating comprehensive test suite...\n');

    const startTime = Date.now();

    try {
      // Run all generators in parallel for maximum speed
      await Promise.all([
        this.generateDynamicTestCases({
          feature: options.feature,
          count: options.testCases || 30
        }),
        this.generateEdgeCases({
          dataType: 'string',
          count: options.edgeCases || 20
        }),
        this.generatePerformanceTestData({
          scenario: options.feature,
          dataPoints: options.performanceTests || 10000
        }),
        this.generateSecurityTestData({
          count: options.securityTests || 30
        }),
        this.generatePipelineData(),
        this.generateRegressionTests()
      ]);

      const duration = Date.now() - startTime;

      console.log(`\n✅ Comprehensive test suite generated in ${duration}ms`);
      console.log(`📁 Output directory: ${path.resolve(this.config.outputDir!)}`);
    } catch (error) {
      console.error('\n❌ Failed to generate test suite:', error);
      throw error;
    }
  }

  /**
   * Save result to file
   */
  private async saveResult(name: string, result: GenerationResult): Promise<void> {
    try {
      await fs.mkdir(this.config.outputDir!, { recursive: true });

      const filepath = path.join(this.config.outputDir!, `${name}.json`);
      await fs.writeFile(filepath, JSON.stringify(result.data, null, 2), 'utf-8');

      const metadataPath = path.join(this.config.outputDir!, `${name}.metadata.json`);
      await fs.writeFile(metadataPath, JSON.stringify(result.metadata, null, 2), 'utf-8');
    } catch (error) {
      console.error(`Failed to save ${name}:`, error);
      throw error;
    }
  }
}

/**
 * Example: GitHub Actions Integration
 */
async function githubActionsPipelineTest() {
  const tester = new PipelineTester({
    outputDir: process.env.GITHUB_WORKSPACE + '/test-data',
    seed: process.env.GITHUB_SHA
  });

  await tester.generateComprehensiveTestSuite({
    feature: process.env.FEATURE_NAME || 'default',
    testCases: 50,
    edgeCases: 30,
    performanceTests: 20000,
    securityTests: 40
  });
}

/**
 * Example: GitLab CI Integration
 */
async function gitlabCIPipelineTest() {
  const tester = new PipelineTester({
    outputDir: process.env.CI_PROJECT_DIR + '/test-data',
    seed: process.env.CI_COMMIT_SHORT_SHA
  });

  await tester.generatePipelineData({
    stages: ['build', 'test', 'security', 'deploy'],
    jobsPerStage: 15
  });
}

/**
 * Example: Jenkins Pipeline Integration
 */
async function jenkinsPipelineTest() {
  const tester = new PipelineTester({
    outputDir: process.env.WORKSPACE + '/test-data',
    seed: process.env.BUILD_NUMBER
  });

  await tester.generateComprehensiveTestSuite({
    feature: process.env.JOB_NAME || 'default'
  });
}

// Export for use in CI/CD scripts
export {
  githubActionsPipelineTest,
  gitlabCIPipelineTest,
  jenkinsPipelineTest
};

// Run if called directly
if (import.meta.url === `file://${process.argv[1]}`) {
  const tester = new PipelineTester();
  tester.generateComprehensiveTestSuite({ feature: 'example' }).catch(console.error);
}
