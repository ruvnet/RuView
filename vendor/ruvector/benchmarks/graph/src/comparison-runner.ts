/**
 * Comparison runner for RuVector vs Neo4j benchmarks
 * Executes benchmarks on both systems and compares results
 */

import { exec } from 'child_process';
import { promisify } from 'util';
import { readFileSync, writeFileSync, existsSync } from 'fs';
import { join } from 'path';

const execAsync = promisify(exec);

export interface BenchmarkMetrics {
  system: 'ruvector' | 'neo4j';
  scenario: string;
  operation: string;
  duration_ms: number;
  throughput_ops: number;
  memory_mb: number;
  cpu_percent: number;
  latency_p50: number;
  latency_p95: number;
  latency_p99: number;
}

export interface ComparisonResult {
  scenario: string;
  operation: string;
  ruvector: BenchmarkMetrics;
  neo4j: BenchmarkMetrics;
  speedup: number;
  memory_improvement: number;
  verdict: 'pass' | 'fail';
}

/**
 * Run RuVector benchmarks
 */
async function runRuVectorBenchmarks(scenario: string): Promise<BenchmarkMetrics[]> {
  console.log(`Running RuVector benchmarks for ${scenario}...`);

  try {
    // Run Rust benchmarks
    const { stdout, stderr } = await execAsync(
      `cargo bench --bench graph_bench -- --save-baseline ${scenario}`,
      { cwd: '/home/user/ruvector/crates/ruvector-graph' }
    );

    console.log('RuVector benchmark output:', stdout);

    // Parse criterion output
    const metrics = parseCriterionOutput(stdout, 'ruvector', scenario);

    return metrics;
  } catch (error) {
    console.error('Error running RuVector benchmarks:', error);
    throw error;
  }
}

/**
 * Run Neo4j benchmarks
 */
async function runNeo4jBenchmarks(scenario: string): Promise<BenchmarkMetrics[]> {
  console.log(`Running Neo4j benchmarks for ${scenario}...`);

  // Check if Neo4j is available
  try {
    await execAsync('which cypher-shell');
  } catch {
    console.warn('Neo4j not available, using baseline metrics');
    return loadBaselineMetrics('neo4j', scenario);
  }

  try {
    // Run equivalent Neo4j queries
    const queries = generateNeo4jQuery(scenario);
    const metrics: BenchmarkMetrics[] = [];

    for (const query of queries) {
      const start = Date.now();

      await execAsync(
        `cypher-shell -u neo4j -p password "${query.cypher}"`,
        { timeout: 300000 }
      );

      const duration = Date.now() - start;

      metrics.push({
        system: 'neo4j',
        scenario,
        operation: query.operation,
        duration_ms: duration,
        throughput_ops: query.count / (duration / 1000),
        memory_mb: 0, // Would need Neo4j metrics API
        cpu_percent: 0,
        latency_p50: duration,
        latency_p95: 0, // Cannot accurately estimate without percentile data
        latency_p99: 0  // Cannot accurately estimate without percentile data
      });
    }

    return metrics;
  } catch (error) {
    console.error('Error running Neo4j benchmarks:', error);
    return loadBaselineMetrics('neo4j', scenario);
  }
}

/**
 * Generate Neo4j Cypher queries for scenario
 */
function generateNeo4jQuery(scenario: string): Array<{ operation: string; cypher: string; count: number }> {
  const queries: Record<string, Array<{ operation: string; cypher: string; count: number }>> = {
    social_network: [
      {
        operation: 'node_creation',
        cypher: 'UNWIND range(1, 1000) AS i CREATE (u:User {id: i, name: "user_" + i})',
        count: 1000
      },
      {
        operation: 'edge_creation',
        cypher: 'MATCH (u1:User), (u2:User) WHERE u1.id < u2.id AND rand() < 0.01 CREATE (u1)-[:FRIENDS_WITH]->(u2)',
        count: 10000
      },
      {
        operation: '1hop_traversal',
        cypher: 'MATCH (u:User {id: 500})-[:FRIENDS_WITH]-(friend) RETURN count(friend)',
        count: 1
      },
      {
        operation: '2hop_traversal',
        cypher: 'MATCH (u:User {id: 500})-[:FRIENDS_WITH*..2]-(friend) RETURN count(DISTINCT friend)',
        count: 1
      },
      {
        operation: 'aggregation',
        cypher: 'MATCH (u:User) RETURN avg(u.age) AS avgAge',
        count: 1
      }
    ],
    knowledge_graph: [
      {
        operation: 'multi_hop',
        cypher: 'MATCH (p:Person)-[:WORKS_AT]->(o:Organization)-[:LOCATED_IN]->(l:Location) RETURN p.name, o.name, l.name LIMIT 100',
        count: 100
      },
      {
        operation: 'path_finding',
        cypher: 'MATCH path = shortestPath((e1:Entity)-[*]-(e2:Entity)) WHERE id(e1) = 0 AND id(e2) = 1000 RETURN length(path)',
        count: 1
      }
    ],
    temporal_events: [
      {
        operation: 'time_range_query',
        cypher: 'MATCH (e:Event) WHERE e.timestamp > datetime() - duration({days: 7}) RETURN count(e)',
        count: 1
      },
      {
        operation: 'state_transition',
        cypher: 'MATCH (e1:Event)-[:TRANSITIONS_TO]->(e2:Event) RETURN count(*)',
        count: 1
      }
    ]
  };

  return queries[scenario] || [];
}

/**
 * Parse Criterion benchmark output
 */
function parseCriterionOutput(output: string, system: 'ruvector' | 'neo4j', scenario: string): BenchmarkMetrics[] {
  const metrics: BenchmarkMetrics[] = [];

  // Parse criterion output format
  const lines = output.split('\n');
  let currentOperation = '';

  for (const line of lines) {
    // Match benchmark group names
    if (line.includes('Benchmarking')) {
      const match = line.match(/Benchmarking (.+)/);
      if (match) {
        currentOperation = match[1];
      }
    }

    // Match timing results
    if (line.includes('time:') && currentOperation) {
      const timeMatch = line.match(/time:\s+\[(.+?)\s+(.+?)\s+(.+?)\]/);
      if (timeMatch) {
        const p50 = parseFloat(timeMatch[2]);

        metrics.push({
          system,
          scenario,
          operation: currentOperation,
          duration_ms: p50,
          throughput_ops: 1000 / p50,
          memory_mb: 0,
          cpu_percent: 0,
          latency_p50: p50,
          latency_p95: 0, // Would need to parse from criterion percentile output
          latency_p99: 0  // Would need to parse from criterion percentile output
        });
      }
    }
  }

  return metrics;
}

/**
 * Load baseline metrics (pre-recorded Neo4j results)
 */
function loadBaselineMetrics(system: string, scenario: string): BenchmarkMetrics[] {
  const baselinePath = join(__dirname, '../data/baselines', `${system}_${scenario}.json`);

  if (existsSync(baselinePath)) {
    const data = readFileSync(baselinePath, 'utf-8');
    return JSON.parse(data);
  }

  // Error: no baseline data available
  throw new Error(
    `No baseline data available for ${system} ${scenario}. ` +
    `Cannot run comparison without actual measured data. ` +
    `Please run benchmarks on both systems first and save results to ${baselinePath}`
  );
}

/**
 * Compare RuVector vs Neo4j results
 */
function compareResults(
  ruvectorMetrics: BenchmarkMetrics[],
  neo4jMetrics: BenchmarkMetrics[]
): ComparisonResult[] {
  const results: ComparisonResult[] = [];

  // Match operations between systems
  for (const rvMetric of ruvectorMetrics) {
    const neoMetric = neo4jMetrics.find(m =>
      m.operation === rvMetric.operation ||
      m.operation.includes(rvMetric.operation.split('_')[0])
    );

    if (!neoMetric) continue;

    const speedup = neoMetric.duration_ms / rvMetric.duration_ms;
    const memoryImprovement = (neoMetric.memory_mb - rvMetric.memory_mb) / neoMetric.memory_mb;

    // Pass if RuVector is 10x faster OR uses 50% less memory
    const verdict = speedup >= 10 || memoryImprovement >= 0.5 ? 'pass' : 'fail';

    results.push({
      scenario: rvMetric.scenario,
      operation: rvMetric.operation,
      ruvector: rvMetric,
      neo4j: neoMetric,
      speedup,
      memory_improvement: memoryImprovement,
      verdict
    });
  }

  return results;
}

/**
 * Run comparison benchmark
 */
export async function runComparison(scenario: string): Promise<ComparisonResult[]> {
  console.log(`\n=== Running Comparison: ${scenario} ===\n`);

  // Run both benchmarks in parallel
  const [ruvectorMetrics, neo4jMetrics] = await Promise.all([
    runRuVectorBenchmarks(scenario),
    runNeo4jBenchmarks(scenario)
  ]);

  // Compare results
  const comparison = compareResults(ruvectorMetrics, neo4jMetrics);

  // Print summary
  console.log('\n=== Comparison Results ===\n');
  console.table(comparison.map(r => ({
    Operation: r.operation,
    'RuVector (ms)': r.ruvector.duration_ms.toFixed(2),
    'Neo4j (ms)': r.neo4j.duration_ms.toFixed(2),
    'Speedup': `${r.speedup.toFixed(2)}x`,
    'Verdict': r.verdict === 'pass' ? '✅ PASS' : '❌ FAIL'
  })));

  // Save results
  const outputPath = join(__dirname, '../results/graph', `${scenario}_comparison.json`);
  writeFileSync(outputPath, JSON.stringify(comparison, null, 2));
  console.log(`\nResults saved to: ${outputPath}`);

  return comparison;
}

/**
 * Run all comparisons
 */
export async function runAllComparisons(): Promise<void> {
  const scenarios = ['social_network', 'knowledge_graph', 'temporal_events'];

  for (const scenario of scenarios) {
    await runComparison(scenario);
  }

  console.log('\n=== All Comparisons Complete ===');
}

// Run if called directly
if (require.main === module) {
  const scenario = process.argv[2] || 'all';

  if (scenario === 'all') {
    runAllComparisons().catch(console.error);
  } else {
    runComparison(scenario).catch(console.error);
  }
}
