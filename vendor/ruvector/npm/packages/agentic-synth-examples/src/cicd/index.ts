/**
 * CI/CD Data Generator - Pipeline testing and deployment simulation
 *
 * Generates realistic CI/CD pipeline data including build results, test outcomes,
 * deployment scenarios, performance metrics, and monitoring alerts. Perfect for
 * testing DevOps tools and ML models for CI/CD optimization.
 *
 * @packageDocumentation
 */

import { EventEmitter } from 'events';
import { AgenticSynth, SynthConfig, GenerationResult, EventOptions } from '@ruvector/agentic-synth';

/**
 * Pipeline execution status
 */
export type PipelineStatus = 'pending' | 'running' | 'success' | 'failed' | 'cancelled' | 'skipped';

/**
 * Pipeline stage types
 */
export type StageType = 'build' | 'test' | 'lint' | 'security-scan' | 'deploy' | 'rollback';

/**
 * Deployment environment
 */
export type Environment = 'development' | 'staging' | 'production' | 'test';

/**
 * Pipeline execution data
 */
export interface PipelineExecution {
  id: string;
  pipelineName: string;
  trigger: 'push' | 'pull-request' | 'schedule' | 'manual';
  branch: string;
  commit: string;
  author: string;
  startTime: Date;
  endTime?: Date;
  duration?: number; // milliseconds
  status: PipelineStatus;
  stages: StageExecution[];
  artifacts?: string[];
}

/**
 * Stage execution data
 */
export interface StageExecution {
  name: string;
  type: StageType;
  status: PipelineStatus;
  startTime: Date;
  endTime?: Date;
  duration?: number;
  logs?: string[];
  errorMessage?: string;
  metrics?: Record<string, number>;
}

/**
 * Test execution results
 */
export interface TestResults {
  id: string;
  pipelineId: string;
  framework: string;
  totalTests: number;
  passed: number;
  failed: number;
  skipped: number;
  duration: number;
  coverage?: number; // Percentage
  failedTests?: Array<{
    name: string;
    error: string;
    stackTrace?: string;
  }>;
}

/**
 * Deployment record
 */
export interface DeploymentRecord {
  id: string;
  pipelineId: string;
  environment: Environment;
  version: string;
  status: 'deploying' | 'deployed' | 'failed' | 'rolled-back';
  startTime: Date;
  endTime?: Date;
  deployedBy: string;
  rollbackReason?: string;
  healthChecks?: Array<{
    name: string;
    status: 'healthy' | 'unhealthy';
    message?: string;
  }>;
}

/**
 * Performance metrics
 */
export interface PerformanceMetrics {
  timestamp: Date;
  pipelineId: string;
  cpuUsage: number; // Percentage
  memoryUsage: number; // MB
  diskIO: number; // MB/s
  networkIO: number; // MB/s
  buildTime: number; // seconds
  testTime: number; // seconds
}

/**
 * Monitoring alert
 */
export interface MonitoringAlert {
  id: string;
  timestamp: Date;
  severity: 'info' | 'warning' | 'error' | 'critical';
  source: string;
  title: string;
  message: string;
  environment: Environment;
  resolved: boolean;
  resolvedAt?: Date;
}

/**
 * CI/CD configuration
 */
export interface CICDConfig extends Partial<SynthConfig> {
  pipelineNames?: string[];
  environments?: Environment[];
  failureRate?: number; // 0-1, probability of failures
  includePerformanceData?: boolean;
  includeAlerts?: boolean;
}

/**
 * CI/CD Data Generator for pipeline testing and DevOps analytics
 *
 * Features:
 * - Pipeline execution simulation
 * - Test result generation
 * - Deployment scenario creation
 * - Performance metrics tracking
 * - Monitoring alert generation
 * - Build artifact management
 *
 * @example
 * ```typescript
 * const generator = new CICDDataGenerator({
 *   provider: 'gemini',
 *   apiKey: process.env.GEMINI_API_KEY,
 *   pipelineNames: ['backend-api', 'frontend-ui', 'mobile-app'],
 *   failureRate: 0.15,
 *   includePerformanceData: true
 * });
 *
 * // Generate pipeline executions
 * const pipelines = await generator.generatePipelineExecutions({
 *   count: 50,
 *   dateRange: { start: new Date('2024-01-01'), end: new Date() }
 * });
 *
 * // Generate test results
 * const tests = await generator.generateTestResults(pipelines[0].id);
 *
 * // Simulate deployment
 * const deployment = await generator.generateDeployment({
 *   pipelineId: pipelines[0].id,
 *   environment: 'production'
 * });
 * ```
 */
export class CICDDataGenerator extends EventEmitter {
  private synth: AgenticSynth;
  private config: CICDConfig;
  private executions: PipelineExecution[] = [];
  private deployments: DeploymentRecord[] = [];
  private alerts: MonitoringAlert[] = [];
  private metrics: PerformanceMetrics[] = [];

  constructor(config: CICDConfig = {}) {
    super();

    this.config = {
      provider: config.provider || 'gemini',
      apiKey: config.apiKey || process.env.GEMINI_API_KEY || '',
      ...(config.model && { model: config.model }),
      cacheStrategy: config.cacheStrategy || 'memory',
      cacheTTL: config.cacheTTL || 3600,
      maxRetries: config.maxRetries || 3,
      timeout: config.timeout || 30000,
      streaming: config.streaming || false,
      automation: config.automation || false,
      vectorDB: config.vectorDB || false,
      pipelineNames: config.pipelineNames || ['main-pipeline', 'feature-pipeline'],
      environments: config.environments || ['development', 'staging', 'production'],
      failureRate: config.failureRate ?? 0.1,
      includePerformanceData: config.includePerformanceData ?? true,
      includeAlerts: config.includeAlerts ?? true
    };

    this.synth = new AgenticSynth(this.config);
  }

  /**
   * Generate pipeline executions
   */
  async generatePipelineExecutions(options: {
    count?: number;
    dateRange?: { start: Date; end: Date };
    pipelineName?: string;
  } = {}): Promise<GenerationResult<PipelineExecution>> {
    this.emit('pipelines:generating', { options });

    try {
      const eventOptions: Partial<EventOptions> = {
        count: options.count || 20,
        eventTypes: ['push', 'pull-request', 'schedule', 'manual'],
        distribution: 'poisson',
        timeRange: options.dateRange || {
          start: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000),
          end: new Date()
        }
      };

      const result = await this.synth.generateEvents<{
        trigger: string;
        branch: string;
        commit: string;
        author: string;
      }>(eventOptions);

      const pipelines: PipelineExecution[] = await Promise.all(
        result.data.map(async (event, index) => {
          const pipelineName = options.pipelineName ||
            this.config.pipelineNames[index % this.config.pipelineNames.length];

          const startTime = new Date(Date.now() - Math.random() * 30 * 24 * 60 * 60 * 1000);
          const duration = Math.floor(Math.random() * 600000) + 60000; // 1-10 minutes
          const endTime = new Date(startTime.getTime() + duration);

          // Determine status based on failure rate
          const hasFailed = Math.random() < this.config.failureRate;
          const status: PipelineStatus = hasFailed ? 'failed' : 'success';

          // Generate stages
          const stages = await this.generateStages(status);

          const pipeline: PipelineExecution = {
            id: this.generateId('pipeline'),
            pipelineName,
            trigger: event.trigger as PipelineExecution['trigger'],
            branch: event.branch || 'main',
            commit: event.commit || this.generateCommitHash(),
            author: event.author || 'developer',
            startTime,
            endTime,
            duration,
            status,
            stages,
            artifacts: status === 'success' ? ['app.zip', 'test-results.xml'] : undefined
          };

          return pipeline;
        })
      );

      this.executions.push(...pipelines);

      this.emit('pipelines:generated', {
        count: pipelines.length,
        successRate: pipelines.filter(p => p.status === 'success').length / pipelines.length
      });

      return {
        data: pipelines,
        metadata: result.metadata
      };
    } catch (error) {
      this.emit('pipelines:error', { error });
      throw error;
    }
  }

  /**
   * Generate test results for a pipeline
   */
  async generateTestResults(pipelineId: string): Promise<TestResults> {
    this.emit('tests:generating', { pipelineId });

    const totalTests = Math.floor(Math.random() * 500) + 100;
    const passRate = 1 - this.config.failureRate;
    const passed = Math.floor(totalTests * passRate);
    const failed = Math.floor((totalTests - passed) * 0.8);
    const skipped = totalTests - passed - failed;

    const tests: TestResults = {
      id: this.generateId('test'),
      pipelineId,
      framework: ['jest', 'pytest', 'junit', 'mocha'][Math.floor(Math.random() * 4)],
      totalTests,
      passed,
      failed,
      skipped,
      duration: Math.floor(Math.random() * 300000) + 10000, // 10s - 5min
      coverage: Math.floor(Math.random() * 30) + 70, // 70-100%
      failedTests: failed > 0 ? Array.from({ length: Math.min(failed, 5) }, (_, i) => ({
        name: `test_case_${i + 1}`,
        error: 'AssertionError: Expected true but got false',
        stackTrace: 'at test_case (test.js:42:10)'
      })) : undefined
    };

    this.emit('tests:generated', { testId: tests.id, passed, failed });

    return tests;
  }

  /**
   * Generate deployment record
   */
  async generateDeployment(options: {
    pipelineId: string;
    environment: Environment;
    version?: string;
  }): Promise<DeploymentRecord> {
    this.emit('deployment:generating', { options });

    const startTime = new Date();
    const duration = Math.floor(Math.random() * 180000) + 30000; // 30s - 3min
    const endTime = new Date(startTime.getTime() + duration);

    const isSuccess = Math.random() > this.config.failureRate;

    const deployment: DeploymentRecord = {
      id: this.generateId('deploy'),
      pipelineId: options.pipelineId,
      environment: options.environment,
      version: options.version || `v${Math.floor(Math.random() * 10)}.${Math.floor(Math.random() * 20)}.${Math.floor(Math.random() * 100)}`,
      status: isSuccess ? 'deployed' : 'failed',
      startTime,
      endTime,
      deployedBy: 'ci-bot',
      rollbackReason: !isSuccess ? 'Health checks failed' : undefined,
      healthChecks: [
        { name: 'api-health', status: isSuccess ? 'healthy' : 'unhealthy', message: isSuccess ? 'OK' : 'Connection refused' },
        { name: 'database', status: 'healthy', message: 'OK' },
        { name: 'cache', status: 'healthy', message: 'OK' }
      ]
    };

    this.deployments.push(deployment);

    this.emit('deployment:complete', {
      deploymentId: deployment.id,
      environment: deployment.environment,
      status: deployment.status
    });

    return deployment;
  }

  /**
   * Generate performance metrics
   */
  async generatePerformanceMetrics(pipelineId: string, count: number = 10): Promise<PerformanceMetrics[]> {
    if (!this.config.includePerformanceData) {
      return [];
    }

    this.emit('metrics:generating', { pipelineId, count });

    const metricsData: PerformanceMetrics[] = Array.from({ length: count }, (_, i) => ({
      timestamp: new Date(Date.now() - (count - i) * 60000),
      pipelineId,
      cpuUsage: Math.random() * 80 + 20, // 20-100%
      memoryUsage: Math.random() * 2048 + 512, // 512-2560 MB
      diskIO: Math.random() * 100, // 0-100 MB/s
      networkIO: Math.random() * 50, // 0-50 MB/s
      buildTime: Math.random() * 300 + 30, // 30-330 seconds
      testTime: Math.random() * 180 + 20 // 20-200 seconds
    }));

    this.metrics.push(...metricsData);

    this.emit('metrics:generated', { count: metricsData.length });

    return metricsData;
  }

  /**
   * Generate monitoring alerts
   */
  async generateAlerts(count: number = 5): Promise<MonitoringAlert[]> {
    if (!this.config.includeAlerts) {
      return [];
    }

    this.emit('alerts:generating', { count });

    const alerts: MonitoringAlert[] = Array.from({ length: count }, (_, i) => {
      const timestamp = new Date(Date.now() - Math.random() * 24 * 60 * 60 * 1000);
      const resolved = Math.random() > 0.5;

      return {
        id: this.generateId('alert'),
        timestamp,
        severity: ['info', 'warning', 'error', 'critical'][Math.floor(Math.random() * 4)] as MonitoringAlert['severity'],
        source: 'pipeline-monitor',
        title: ['High CPU usage', 'Memory leak detected', 'Build timeout', 'Test failures'][Math.floor(Math.random() * 4)],
        message: 'Alert details and context',
        environment: this.config.environments[Math.floor(Math.random() * this.config.environments.length)],
        resolved,
        resolvedAt: resolved ? new Date(timestamp.getTime() + Math.random() * 3600000) : undefined
      };
    });

    this.alerts.push(...alerts);

    this.emit('alerts:generated', { count: alerts.length });

    return alerts;
  }

  /**
   * Get CI/CD statistics
   */
  getStatistics(): {
    totalExecutions: number;
    successRate: number;
    avgDuration: number;
    totalDeployments: number;
    deploymentSuccessRate: number;
    activeAlerts: number;
  } {
    const successfulExecutions = this.executions.filter(e => e.status === 'success').length;
    const totalDuration = this.executions.reduce((sum, e) => sum + (e.duration || 0), 0);
    const successfulDeployments = this.deployments.filter(d => d.status === 'deployed').length;
    const activeAlerts = this.alerts.filter(a => !a.resolved).length;

    return {
      totalExecutions: this.executions.length,
      successRate: this.executions.length > 0 ? successfulExecutions / this.executions.length : 0,
      avgDuration: this.executions.length > 0 ? totalDuration / this.executions.length : 0,
      totalDeployments: this.deployments.length,
      deploymentSuccessRate: this.deployments.length > 0 ? successfulDeployments / this.deployments.length : 0,
      activeAlerts
    };
  }

  /**
   * Export pipeline data to JSON
   */
  exportPipelineData(): string {
    return JSON.stringify({
      executions: this.executions,
      deployments: this.deployments,
      alerts: this.alerts,
      metrics: this.metrics
    }, null, 2);
  }

  /**
   * Reset generator state
   */
  reset(): void {
    this.executions = [];
    this.deployments = [];
    this.alerts = [];
    this.metrics = [];

    this.emit('reset', { timestamp: new Date() });
  }

  /**
   * Generate pipeline stages
   */
  private async generateStages(finalStatus: PipelineStatus): Promise<StageExecution[]> {
    const stageTypes: StageType[] = ['build', 'lint', 'test', 'security-scan', 'deploy'];
    const stages: StageExecution[] = [];

    let currentTime = Date.now();

    for (let i = 0; i < stageTypes.length; i++) {
      const startTime = new Date(currentTime);
      const duration = Math.floor(Math.random() * 120000) + 10000; // 10s - 2min
      const endTime = new Date(currentTime + duration);

      // Fail at random stage if pipeline should fail
      const shouldFail = finalStatus === 'failed' && i === Math.floor(Math.random() * stageTypes.length);
      const status: PipelineStatus = shouldFail ? 'failed' : 'success';

      stages.push({
        name: stageTypes[i],
        type: stageTypes[i],
        status,
        startTime,
        endTime,
        duration,
        logs: [`Stage ${stageTypes[i]} started`, `Stage ${stageTypes[i]} completed`],
        errorMessage: shouldFail ? 'Stage failed with error' : undefined,
        metrics: {
          cpuUsage: Math.random() * 100,
          memoryUsage: Math.random() * 2048
        }
      });

      currentTime += duration;

      // Stop at failed stage
      if (shouldFail) break;
    }

    return stages;
  }

  /**
   * Generate commit hash
   */
  private generateCommitHash(): string {
    return Array.from({ length: 40 }, () =>
      Math.floor(Math.random() * 16).toString(16)
    ).join('');
  }

  /**
   * Generate unique ID
   */
  private generateId(prefix: string): string {
    return `${prefix}_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
  }
}

/**
 * Create a new CI/CD data generator instance
 */
export function createCICDDataGenerator(config?: CICDConfig): CICDDataGenerator {
  return new CICDDataGenerator(config);
}
