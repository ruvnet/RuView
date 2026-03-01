/**
 * Benchmark Commands
 * CLI commands for performance benchmarking
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import type { RuVectorClient } from '../client.js';

export interface BenchmarkRunOptions {
  type: 'vector' | 'attention' | 'gnn' | 'all';
  size: string;
  dim: string;
}

export interface BenchmarkReportOptions {
  format: 'json' | 'table' | 'markdown';
}

interface BenchmarkResult {
  name: string;
  operations: number;
  totalTime: number;
  avgTime: number;
  opsPerSec: number;
  p50: number;
  p95: number;
  p99: number;
}

export class BenchmarkCommands {
  static async run(
    client: RuVectorClient,
    options: BenchmarkRunOptions
  ): Promise<void> {
    const spinner = ora('Running benchmarks...').start();

    try {
      await client.connect();

      const size = parseInt(options.size);
      const dim = parseInt(options.dim);

      const results: BenchmarkResult[] = [];

      // Vector benchmarks
      if (options.type === 'vector' || options.type === 'all') {
        spinner.text = 'Running vector benchmarks...';

        const vectorResult = await client.runBenchmark('vector', size, dim);
        results.push({
          name: 'Vector Search',
          operations: size,
          totalTime: vectorResult.total_time as number,
          avgTime: vectorResult.avg_time as number,
          opsPerSec: vectorResult.ops_per_sec as number,
          p50: vectorResult.p50 as number,
          p95: vectorResult.p95 as number,
          p99: vectorResult.p99 as number,
        });
      }

      // Attention benchmarks
      if (options.type === 'attention' || options.type === 'all') {
        spinner.text = 'Running attention benchmarks...';

        const attentionResult = await client.runBenchmark('attention', size, dim);
        results.push({
          name: 'Attention',
          operations: size,
          totalTime: attentionResult.total_time as number,
          avgTime: attentionResult.avg_time as number,
          opsPerSec: attentionResult.ops_per_sec as number,
          p50: attentionResult.p50 as number,
          p95: attentionResult.p95 as number,
          p99: attentionResult.p99 as number,
        });
      }

      // GNN benchmarks
      if (options.type === 'gnn' || options.type === 'all') {
        spinner.text = 'Running GNN benchmarks...';

        const gnnResult = await client.runBenchmark('gnn', size, dim);
        results.push({
          name: 'GNN Forward',
          operations: size,
          totalTime: gnnResult.total_time as number,
          avgTime: gnnResult.avg_time as number,
          opsPerSec: gnnResult.ops_per_sec as number,
          p50: gnnResult.p50 as number,
          p95: gnnResult.p95 as number,
          p99: gnnResult.p99 as number,
        });
      }

      spinner.succeed(chalk.green('Benchmarks completed'));

      // Display results
      console.log(chalk.bold.blue('\nBenchmark Results:'));
      console.log(chalk.gray('─'.repeat(70)));
      console.log(`  ${chalk.gray('Dataset Size:')} ${size.toLocaleString()}`);
      console.log(`  ${chalk.gray('Dimensions:')} ${dim}`);

      const table = new Table({
        head: [
          chalk.cyan('Benchmark'),
          chalk.cyan('Ops/sec'),
          chalk.cyan('Avg (ms)'),
          chalk.cyan('P50 (ms)'),
          chalk.cyan('P95 (ms)'),
          chalk.cyan('P99 (ms)')
        ],
        colWidths: [18, 12, 12, 12, 12, 12]
      });

      for (const result of results) {
        table.push([
          result.name,
          result.opsPerSec.toFixed(0),
          result.avgTime.toFixed(3),
          result.p50.toFixed(3),
          result.p95.toFixed(3),
          result.p99.toFixed(3)
        ]);
      }

      console.log(table.toString());

      // Summary
      const totalOps = results.reduce((sum, r) => sum + r.opsPerSec, 0);
      console.log(`\n  ${chalk.green('Total Throughput:')} ${totalOps.toFixed(0)} ops/sec`);
    } catch (err) {
      spinner.fail(chalk.red('Benchmark failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async report(
    client: RuVectorClient,
    options: BenchmarkReportOptions
  ): Promise<void> {
    const spinner = ora('Generating benchmark report...').start();

    try {
      await client.connect();

      // Get historical benchmark results
      const results = await client.query<{
        id: number;
        benchmark_type: string;
        created_at: string;
        metrics: Record<string, unknown>;
      }>(
        'SELECT * FROM benchmark_results ORDER BY created_at DESC LIMIT 10'
      );

      spinner.stop();

      if (results.length === 0) {
        console.log(chalk.yellow('No benchmark results found'));
        console.log(chalk.gray('Run benchmarks first: ruvector-pg bench run'));
        return;
      }

      if (options.format === 'json') {
        console.log(JSON.stringify(results, null, 2));
        return;
      }

      if (options.format === 'markdown') {
        console.log('# Benchmark Report\n');
        console.log('| Type | Date | Ops/sec | Avg Time |');
        console.log('|------|------|---------|----------|');

        for (const result of results) {
          const metrics = result.metrics as { ops_per_sec?: number; avg_time?: number };
          console.log(
            `| ${result.benchmark_type} | ${result.created_at} | ` +
            `${metrics.ops_per_sec?.toFixed(0) || 'N/A'} | ` +
            `${metrics.avg_time?.toFixed(3) || 'N/A'}ms |`
          );
        }
        return;
      }

      // Default: table format
      console.log(chalk.bold.blue('\nBenchmark History:'));
      console.log(chalk.gray('─'.repeat(70)));

      const table = new Table({
        head: [
          chalk.cyan('ID'),
          chalk.cyan('Type'),
          chalk.cyan('Date'),
          chalk.cyan('Ops/sec'),
          chalk.cyan('Avg (ms)')
        ],
        colWidths: [8, 15, 25, 12, 12]
      });

      for (const result of results) {
        const metrics = result.metrics as { ops_per_sec?: number; avg_time?: number };
        table.push([
          String(result.id),
          result.benchmark_type,
          result.created_at,
          metrics.ops_per_sec?.toFixed(0) || 'N/A',
          metrics.avg_time?.toFixed(3) || 'N/A'
        ]);
      }

      console.log(table.toString());
    } catch (err) {
      spinner.fail(chalk.red('Failed to generate report'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showInfo(): void {
    console.log(chalk.bold.blue('\nBenchmark System:'));
    console.log(chalk.gray('─'.repeat(50)));

    console.log(`
${chalk.yellow('Available Benchmarks:')}

  ${chalk.green('vector')}    - Vector similarity search performance
                HNSW index operations, cosine/L2/IP distances

  ${chalk.green('attention')} - Attention mechanism throughput
                Scaled dot-product, multi-head, flash attention

  ${chalk.green('gnn')}       - Graph Neural Network performance
                GCN, GraphSAGE, GAT, GIN forward passes

  ${chalk.green('all')}       - Run all benchmarks sequentially

${chalk.yellow('Options:')}

  ${chalk.gray('-s, --size')}   Dataset size (default: 10000)
  ${chalk.gray('-d, --dim')}    Vector dimensions (default: 384)

${chalk.yellow('Examples:')}

  ${chalk.gray('# Run all benchmarks with 100k vectors')}
  ruvector-pg bench run -t all -s 100000

  ${chalk.gray('# Run vector benchmark with 768 dimensions')}
  ruvector-pg bench run -t vector -d 768

  ${chalk.gray('# Generate markdown report')}
  ruvector-pg bench report -f markdown
`);
  }
}

export default BenchmarkCommands;
