#!/usr/bin/env node
/**
 * Benchmark Runner for RuVector
 *
 * Orchestrates benchmark execution across multiple scenarios and regions
 */

import { execSync, spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { SCENARIOS, Scenario, getScenarioGroup } from './benchmark-scenarios';
import { MetricsCollector, ComprehensiveMetrics, collectFromK6Output } from './metrics-collector';
import { ResultsAnalyzer, AnalysisReport } from './results-analyzer';

// Configuration
interface RunnerConfig {
  outputDir: string;
  k6Binary: string;
  parallelScenarios: number;
  enableHooks: boolean;
  regions: string[];
  baseUrl: string;
  slack WebhookUrl?: string;
  emailNotification?: string;
}

interface TestRun {
  id: string;
  scenario: Scenario;
  status: 'pending' | 'running' | 'completed' | 'failed';
  startTime?: number;
  endTime?: number;
  metrics?: ComprehensiveMetrics;
  analysis?: AnalysisReport;
  error?: string;
}

// Main runner class
export class BenchmarkRunner {
  private config: RunnerConfig;
  private runs: Map<string, TestRun>;
  private resultsDir: string;

  constructor(config: Partial<RunnerConfig> = {}) {
    this.config = {
      outputDir: config.outputDir || './results',
      k6Binary: config.k6Binary || 'k6',
      parallelScenarios: config.parallelScenarios || 1,
      enableHooks: config.enableHooks !== false,
      regions: config.regions || ['all'],
      baseUrl: config.baseUrl || 'http://localhost:8080',
      slackWebhookUrl: config.slackWebhookUrl,
      emailNotification: config.emailNotification,
    };

    this.runs = new Map();
    this.resultsDir = path.join(this.config.outputDir, `run-${Date.now()}`);

    // Create output directories
    if (!fs.existsSync(this.resultsDir)) {
      fs.mkdirSync(this.resultsDir, { recursive: true });
    }
  }

  // Run a single scenario
  async runScenario(scenarioName: string): Promise<TestRun> {
    const scenario = SCENARIOS[scenarioName];
    if (!scenario) {
      throw new Error(`Scenario not found: ${scenarioName}`);
    }

    const runId = `${scenarioName}-${Date.now()}`;
    const run: TestRun = {
      id: runId,
      scenario,
      status: 'pending',
    };

    this.runs.set(runId, run);

    try {
      console.log(`\n${'='.repeat(80)}`);
      console.log(`Starting scenario: ${scenario.name}`);
      console.log(`Description: ${scenario.description}`);
      console.log(`Expected duration: ${scenario.duration}`);
      console.log(`${'='.repeat(80)}\n`);

      // Execute pre-task hook
      if (this.config.enableHooks && scenario.preTestHook) {
        console.log('Executing pre-task hook...');
        execSync(scenario.preTestHook, { stdio: 'inherit' });
      }

      run.status = 'running';
      run.startTime = Date.now();

      // Prepare K6 test file
      const testFile = this.prepareTestFile(scenario);

      // Run K6
      const outputFile = path.join(this.resultsDir, `${runId}-raw.json`);
      await this.executeK6(testFile, outputFile, scenario);

      // Collect metrics
      console.log('Collecting metrics...');
      const collector = collectFromK6Output(outputFile);
      const metrics = collector.generateReport(runId, scenarioName);

      // Save metrics
      const metricsFile = path.join(this.resultsDir, `${runId}-metrics.json`);
      collector.save(metricsFile, metrics);

      // Analyze results
      console.log('Analyzing results...');
      const analyzer = new ResultsAnalyzer(this.resultsDir);
      const analysis = analyzer.generateReport(metrics);

      // Save analysis
      const analysisFile = path.join(this.resultsDir, `${runId}-analysis.json`);
      analyzer.save(analysisFile, analysis);

      // Generate markdown report
      const markdown = analyzer.generateMarkdown(analysis);
      const markdownFile = path.join(this.resultsDir, `${runId}-report.md`);
      fs.writeFileSync(markdownFile, markdown);

      // Export CSV
      collector.exportCSV(`${runId}-metrics.csv`);

      run.status = 'completed';
      run.endTime = Date.now();
      run.metrics = metrics;
      run.analysis = analysis;

      // Execute post-task hook
      if (this.config.enableHooks && scenario.postTestHook) {
        console.log('Executing post-task hook...');
        execSync(scenario.postTestHook, { stdio: 'inherit' });
      }

      // Send notifications
      await this.sendNotifications(run);

      console.log(`\n${'='.repeat(80)}`);
      console.log(`Scenario completed: ${scenario.name}`);
      console.log(`Status: ${run.status}`);
      console.log(`Duration: ${((run.endTime - run.startTime) / 1000 / 60).toFixed(2)} minutes`);
      console.log(`Overall Score: ${analysis.score.overall}/100`);
      console.log(`SLA Compliance: ${analysis.slaCompliance.met ? 'PASSED' : 'FAILED'}`);
      console.log(`${'='.repeat(80)}\n`);

    } catch (error) {
      run.status = 'failed';
      run.endTime = Date.now();
      run.error = error instanceof Error ? error.message : String(error);

      console.error(`\nScenario failed: ${scenario.name}`);
      console.error(`Error: ${run.error}\n`);

      await this.sendNotifications(run);
    }

    return run;
  }

  // Run multiple scenarios
  async runScenarios(scenarioNames: string[]): Promise<Map<string, TestRun>> {
    console.log(`\nRunning ${scenarioNames.length} scenarios...`);
    console.log(`Parallel execution: ${this.config.parallelScenarios}`);
    console.log(`Output directory: ${this.resultsDir}\n`);

    const results = new Map<string, TestRun>();

    // Run scenarios in batches
    for (let i = 0; i < scenarioNames.length; i += this.config.parallelScenarios) {
      const batch = scenarioNames.slice(i, i + this.config.parallelScenarios);

      console.log(`\nBatch ${Math.floor(i / this.config.parallelScenarios) + 1}/${Math.ceil(scenarioNames.length / this.config.parallelScenarios)}`);
      console.log(`Scenarios: ${batch.join(', ')}\n`);

      const promises = batch.map(name => this.runScenario(name));
      const batchResults = await Promise.allSettled(promises);

      batchResults.forEach((result, index) => {
        const scenarioName = batch[index];
        if (result.status === 'fulfilled') {
          results.set(scenarioName, result.value);
        } else {
          console.error(`Failed to run scenario ${scenarioName}:`, result.reason);
        }
      });
    }

    // Generate summary report
    this.generateSummaryReport(results);

    return results;
  }

  // Run scenario group
  async runGroup(groupName: string): Promise<Map<string, TestRun>> {
    const scenarios = getScenarioGroup(groupName as any);
    if (scenarios.length === 0) {
      throw new Error(`Scenario group not found: ${groupName}`);
    }

    console.log(`\nRunning scenario group: ${groupName}`);
    console.log(`Scenarios: ${scenarios.join(', ')}\n`);

    return this.runScenarios(scenarios);
  }

  // Prepare K6 test file
  private prepareTestFile(scenario: Scenario): string {
    const testContent = `
import { check, sleep } from 'k6';
import http from 'k6/http';
import { Trend, Counter, Gauge, Rate } from 'k6/metrics';

// Import scenario configuration
const scenarioConfig = ${JSON.stringify(scenario.config, null, 2)};
const k6Options = ${JSON.stringify(scenario.k6Options, null, 2)};

// Export options
export const options = k6Options;

// Custom metrics
const queryLatency = new Trend('query_latency', true);
const errorRate = new Rate('error_rate');
const queriesPerSecond = new Counter('queries_per_second');

export default function() {
  const baseUrl = __ENV.BASE_URL || '${this.config.baseUrl}';
  const region = __ENV.REGION || 'unknown';

  const payload = JSON.stringify({
    query_id: \`query_\${Date.now()}_\${__VU}_\${__ITER}\`,
    vector: Array.from({ length: scenarioConfig.vectorDimension }, () => Math.random() * 2 - 1),
    top_k: 10,
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
      'X-Region': region,
      'X-VU': __VU.toString(),
    },
    tags: {
      scenario: '${scenario.name}',
      region: region,
    },
  };

  const startTime = Date.now();
  const response = http.post(\`\${baseUrl}/query\`, payload, params);
  const latency = Date.now() - startTime;

  queryLatency.add(latency);
  queriesPerSecond.add(1);

  const success = check(response, {
    'status is 200': (r) => r.status === 200,
    'has results': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.results && body.results.length > 0;
      } catch {
        return false;
      }
    },
    'latency acceptable': () => latency < 200,
  });

  errorRate.add(!success);

  sleep(parseFloat(scenarioConfig.queryInterval) / 1000);
}

export function setup() {
  console.log('Starting test: ${scenario.name}');
  console.log('Description: ${scenario.description}');
  return { startTime: Date.now() };
}

export function teardown(data) {
  const duration = Date.now() - data.startTime;
  console.log(\`Test completed in \${duration}ms\`);
}
`;

    const testFile = path.join(this.resultsDir, `${scenario.name}-test.js`);
    fs.writeFileSync(testFile, testContent);

    return testFile;
  }

  // Execute K6
  private async executeK6(testFile: string, outputFile: string, scenario: Scenario): Promise<void> {
    return new Promise((resolve, reject) => {
      const args = [
        'run',
        '--out', `json=${outputFile}`,
        '--summary-export', `${outputFile}.summary`,
        testFile,
      ];

      // Add environment variables
      const env = {
        ...process.env,
        BASE_URL: this.config.baseUrl,
      };

      console.log(`Executing: ${this.config.k6Binary} ${args.join(' ')}\n`);

      const k6Process = spawn(this.config.k6Binary, args, {
        env,
        stdio: 'inherit',
      });

      k6Process.on('close', (code) => {
        if (code === 0) {
          resolve();
        } else {
          reject(new Error(`K6 exited with code ${code}`));
        }
      });

      k6Process.on('error', (error) => {
        reject(error);
      });
    });
  }

  // Generate summary report
  private generateSummaryReport(results: Map<string, TestRun>): void {
    let summary = `# Benchmark Summary Report\n\n`;
    summary += `**Date:** ${new Date().toISOString()}\n`;
    summary += `**Total Scenarios:** ${results.size}\n`;
    summary += `**Output Directory:** ${this.resultsDir}\n\n`;

    summary += `## Results\n\n`;
    summary += `| Scenario | Status | Duration | Score | SLA |\n`;
    summary += `|----------|--------|----------|-------|-----|\n`;

    for (const [name, run] of results) {
      const duration = run.endTime && run.startTime
        ? ((run.endTime - run.startTime) / 1000 / 60).toFixed(2) + 'm'
        : 'N/A';
      const score = run.analysis?.score.overall || 'N/A';
      const sla = run.analysis?.slaCompliance.met ? '✅' : '❌';

      summary += `| ${name} | ${run.status} | ${duration} | ${score} | ${sla} |\n`;
    }

    summary += `\n## Recommendations\n\n`;

    // Aggregate recommendations
    const allRecommendations = new Map<string, number>();
    for (const run of results.values()) {
      if (run.analysis) {
        for (const rec of run.analysis.recommendations) {
          const key = rec.title;
          allRecommendations.set(key, (allRecommendations.get(key) || 0) + 1);
        }
      }
    }

    for (const [title, count] of Array.from(allRecommendations.entries()).sort((a, b) => b[1] - a[1])) {
      summary += `- ${title} (mentioned in ${count} scenarios)\n`;
    }

    const summaryFile = path.join(this.resultsDir, 'SUMMARY.md');
    fs.writeFileSync(summaryFile, summary);

    console.log(`\nSummary report generated: ${summaryFile}\n`);
  }

  // Send notifications
  private async sendNotifications(run: TestRun): Promise<void> {
    // Slack notification
    if (this.config.slackWebhookUrl) {
      try {
        const message = {
          text: `Benchmark ${run.status}: ${run.scenario.name}`,
          blocks: [
            {
              type: 'section',
              text: {
                type: 'mrkdwn',
                text: `*Benchmark ${run.status.toUpperCase()}*\n*Scenario:* ${run.scenario.name}\n*Status:* ${run.status}\n*Score:* ${run.analysis?.score.overall || 'N/A'}/100`,
              },
            },
          ],
        };

        await fetch(this.config.slackWebhookUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(message),
        });
      } catch (error) {
        console.error('Failed to send Slack notification:', error);
      }
    }
  }
}

// CLI
if (require.main === module) {
  const args = process.argv.slice(2);

  if (args.length === 0) {
    console.log(`
Usage: benchmark-runner.ts <command> [options]

Commands:
  run <scenario>     Run a single scenario
  group <group>      Run a scenario group
  list               List available scenarios

Examples:
  benchmark-runner.ts run baseline_500m
  benchmark-runner.ts group standard_suite
  benchmark-runner.ts list
    `);
    process.exit(1);
  }

  const command = args[0];

  const runner = new BenchmarkRunner({
    baseUrl: process.env.BASE_URL || 'http://localhost:8080',
    parallelScenarios: parseInt(process.env.PARALLEL || '1'),
  });

  (async () => {
    try {
      switch (command) {
        case 'run':
          if (args.length < 2) {
            console.error('Error: Scenario name required');
            process.exit(1);
          }
          await runner.runScenario(args[1]);
          break;

        case 'group':
          if (args.length < 2) {
            console.error('Error: Group name required');
            process.exit(1);
          }
          await runner.runGroup(args[1]);
          break;

        case 'list':
          console.log('\nAvailable scenarios:\n');
          for (const [name, scenario] of Object.entries(SCENARIOS)) {
            console.log(`  ${name.padEnd(30)} - ${scenario.description}`);
          }
          console.log('\nAvailable groups:\n');
          console.log('  quick_validation');
          console.log('  standard_suite');
          console.log('  stress_suite');
          console.log('  reliability_suite');
          console.log('  full_suite\n');
          break;

        default:
          console.error(`Unknown command: ${command}`);
          process.exit(1);
      }
    } catch (error) {
      console.error('Error:', error);
      process.exit(1);
    }
  })();
}

export default BenchmarkRunner;
