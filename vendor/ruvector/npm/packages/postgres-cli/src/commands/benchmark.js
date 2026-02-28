"use strict";
/**
 * Benchmark Commands
 * CLI commands for performance benchmarking
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.BenchmarkCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
class BenchmarkCommands {
    static async run(client, options) {
        const spinner = (0, ora_1.default)('Running benchmarks...').start();
        try {
            await client.connect();
            const size = parseInt(options.size);
            const dim = parseInt(options.dim);
            const results = [];
            // Vector benchmarks
            if (options.type === 'vector' || options.type === 'all') {
                spinner.text = 'Running vector benchmarks...';
                const vectorResult = await client.runBenchmark('vector', size, dim);
                results.push({
                    name: 'Vector Search',
                    operations: size,
                    totalTime: vectorResult.total_time,
                    avgTime: vectorResult.avg_time,
                    opsPerSec: vectorResult.ops_per_sec,
                    p50: vectorResult.p50,
                    p95: vectorResult.p95,
                    p99: vectorResult.p99,
                });
            }
            // Attention benchmarks
            if (options.type === 'attention' || options.type === 'all') {
                spinner.text = 'Running attention benchmarks...';
                const attentionResult = await client.runBenchmark('attention', size, dim);
                results.push({
                    name: 'Attention',
                    operations: size,
                    totalTime: attentionResult.total_time,
                    avgTime: attentionResult.avg_time,
                    opsPerSec: attentionResult.ops_per_sec,
                    p50: attentionResult.p50,
                    p95: attentionResult.p95,
                    p99: attentionResult.p99,
                });
            }
            // GNN benchmarks
            if (options.type === 'gnn' || options.type === 'all') {
                spinner.text = 'Running GNN benchmarks...';
                const gnnResult = await client.runBenchmark('gnn', size, dim);
                results.push({
                    name: 'GNN Forward',
                    operations: size,
                    totalTime: gnnResult.total_time,
                    avgTime: gnnResult.avg_time,
                    opsPerSec: gnnResult.ops_per_sec,
                    p50: gnnResult.p50,
                    p95: gnnResult.p95,
                    p99: gnnResult.p99,
                });
            }
            spinner.succeed(chalk_1.default.green('Benchmarks completed'));
            // Display results
            console.log(chalk_1.default.bold.blue('\nBenchmark Results:'));
            console.log(chalk_1.default.gray('─'.repeat(70)));
            console.log(`  ${chalk_1.default.gray('Dataset Size:')} ${size.toLocaleString()}`);
            console.log(`  ${chalk_1.default.gray('Dimensions:')} ${dim}`);
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('Benchmark'),
                    chalk_1.default.cyan('Ops/sec'),
                    chalk_1.default.cyan('Avg (ms)'),
                    chalk_1.default.cyan('P50 (ms)'),
                    chalk_1.default.cyan('P95 (ms)'),
                    chalk_1.default.cyan('P99 (ms)')
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
            console.log(`\n  ${chalk_1.default.green('Total Throughput:')} ${totalOps.toFixed(0)} ops/sec`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Benchmark failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async report(client, options) {
        const spinner = (0, ora_1.default)('Generating benchmark report...').start();
        try {
            await client.connect();
            // Get historical benchmark results
            const results = await client.query('SELECT * FROM benchmark_results ORDER BY created_at DESC LIMIT 10');
            spinner.stop();
            if (results.length === 0) {
                console.log(chalk_1.default.yellow('No benchmark results found'));
                console.log(chalk_1.default.gray('Run benchmarks first: ruvector-pg bench run'));
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
                    const metrics = result.metrics;
                    console.log(`| ${result.benchmark_type} | ${result.created_at} | ` +
                        `${metrics.ops_per_sec?.toFixed(0) || 'N/A'} | ` +
                        `${metrics.avg_time?.toFixed(3) || 'N/A'}ms |`);
                }
                return;
            }
            // Default: table format
            console.log(chalk_1.default.bold.blue('\nBenchmark History:'));
            console.log(chalk_1.default.gray('─'.repeat(70)));
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('ID'),
                    chalk_1.default.cyan('Type'),
                    chalk_1.default.cyan('Date'),
                    chalk_1.default.cyan('Ops/sec'),
                    chalk_1.default.cyan('Avg (ms)')
                ],
                colWidths: [8, 15, 25, 12, 12]
            });
            for (const result of results) {
                const metrics = result.metrics;
                table.push([
                    String(result.id),
                    result.benchmark_type,
                    result.created_at,
                    metrics.ops_per_sec?.toFixed(0) || 'N/A',
                    metrics.avg_time?.toFixed(3) || 'N/A'
                ]);
            }
            console.log(table.toString());
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to generate report'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showInfo() {
        console.log(chalk_1.default.bold.blue('\nBenchmark System:'));
        console.log(chalk_1.default.gray('─'.repeat(50)));
        console.log(`
${chalk_1.default.yellow('Available Benchmarks:')}

  ${chalk_1.default.green('vector')}    - Vector similarity search performance
                HNSW index operations, cosine/L2/IP distances

  ${chalk_1.default.green('attention')} - Attention mechanism throughput
                Scaled dot-product, multi-head, flash attention

  ${chalk_1.default.green('gnn')}       - Graph Neural Network performance
                GCN, GraphSAGE, GAT, GIN forward passes

  ${chalk_1.default.green('all')}       - Run all benchmarks sequentially

${chalk_1.default.yellow('Options:')}

  ${chalk_1.default.gray('-s, --size')}   Dataset size (default: 10000)
  ${chalk_1.default.gray('-d, --dim')}    Vector dimensions (default: 384)

${chalk_1.default.yellow('Examples:')}

  ${chalk_1.default.gray('# Run all benchmarks with 100k vectors')}
  ruvector-pg bench run -t all -s 100000

  ${chalk_1.default.gray('# Run vector benchmark with 768 dimensions')}
  ruvector-pg bench run -t vector -d 768

  ${chalk_1.default.gray('# Generate markdown report')}
  ruvector-pg bench report -f markdown
`);
    }
}
exports.BenchmarkCommands = BenchmarkCommands;
exports.default = BenchmarkCommands;
//# sourceMappingURL=benchmark.js.map