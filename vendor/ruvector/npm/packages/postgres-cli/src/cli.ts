#!/usr/bin/env node
/**
 * RuVector PostgreSQL CLI
 * Comprehensive command-line interface for the RuVector PostgreSQL extension
 *
 * Features:
 * - Vector operations (dense and sparse)
 * - Attention mechanisms (scaled-dot, multi-head, flash)
 * - Graph Neural Networks (GCN, GraphSAGE)
 * - Graph operations with Cypher queries
 * - Self-learning with ReasoningBank
 * - Hyperbolic geometry (Poincare, Lorentz)
 * - Agent routing (Tiny Dancer)
 * - Vector quantization
 * - Benchmarking
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { createRequire } from 'module';
import { RuVectorClient } from './client.js';
import { VectorCommands } from './commands/vector.js';
import { AttentionCommands } from './commands/attention.js';
import { GnnCommands } from './commands/gnn.js';
import { GraphCommands } from './commands/graph.js';
import { LearningCommands } from './commands/learning.js';
import { BenchmarkCommands } from './commands/benchmark.js';
import { SparseCommands } from './commands/sparse.js';
import { HyperbolicCommands } from './commands/hyperbolic.js';
import { RoutingCommands } from './commands/routing.js';
import { QuantizationCommands } from './commands/quantization.js';
import { InstallCommands } from './commands/install.js';

// Read version from package.json
const require = createRequire(import.meta.url);
const pkg = require('../package.json');

const program = new Command();

program
  .name('ruvector-pg')
  .description('RuVector PostgreSQL CLI - Advanced AI Vector Database Extension')
  .version(pkg.version)
  .option('-c, --connection <string>', 'PostgreSQL connection string', 'postgresql://localhost:5432/ruvector')
  .option('-v, --verbose', 'Enable verbose output');

// ============================================================================
// Vector Operations
// ============================================================================

const vector = program.command('vector').description('Dense vector operations');

vector
  .command('create <name>')
  .description('Create a new vector table')
  .option('-d, --dim <number>', 'Vector dimensions', '384')
  .option('-i, --index <type>', 'Index type (hnsw, ivfflat)', 'hnsw')
  .action(async (name, options) => {
    const client = new RuVectorClient(program.opts().connection);
    await VectorCommands.create(client, name, options);
  });

vector
  .command('insert <table>')
  .description('Insert vectors into a table')
  .option('-f, --file <path>', 'JSON file with vectors')
  .option('-t, --text <content>', 'Text to embed')
  .action(async (table, options) => {
    const client = new RuVectorClient(program.opts().connection);
    await VectorCommands.insert(client, table, options);
  });

vector
  .command('search <table>')
  .description('Search for similar vectors')
  .option('-q, --query <vector>', 'Query vector as JSON array')
  .option('-t, --text <content>', 'Text query to embed and search')
  .option('-k, --top-k <number>', 'Number of results', '10')
  .option('-m, --metric <type>', 'Distance metric (cosine, l2, ip)', 'cosine')
  .action(async (table, options) => {
    const client = new RuVectorClient(program.opts().connection);
    await VectorCommands.search(client, table, options);
  });

vector
  .command('distance')
  .description('Compute distance between two vectors')
  .requiredOption('-a, --a <vector>', 'First vector as JSON array')
  .requiredOption('-b, --b <vector>', 'Second vector as JSON array')
  .option('-m, --metric <type>', 'Distance metric (cosine, l2, ip)', 'cosine')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await VectorCommands.distance(client, options);
  });

vector
  .command('normalize')
  .description('Normalize a vector to unit length')
  .requiredOption('--vector <array>', 'Vector as JSON array')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await VectorCommands.normalize(client, options);
  });

// ============================================================================
// Sparse Vector Operations
// ============================================================================

const sparse = program.command('sparse').description('Sparse vector operations');

sparse
  .command('create')
  .description('Create a sparse vector from indices and values')
  .requiredOption('--indices <array>', 'Non-zero indices as JSON array')
  .requiredOption('--values <array>', 'Values as JSON array')
  .requiredOption('--dim <number>', 'Total dimensionality')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.create(client, options);
  });

sparse
  .command('distance')
  .description('Compute distance between sparse vectors')
  .requiredOption('-a, --a <sparse>', 'First sparse vector')
  .requiredOption('-b, --b <sparse>', 'Second sparse vector')
  .option('-m, --metric <type>', 'Distance metric (dot, cosine, euclidean, manhattan)', 'cosine')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.distance(client, options);
  });

sparse
  .command('bm25')
  .description('Compute BM25 relevance score')
  .requiredOption('--query <sparse>', 'Query sparse vector (IDF weights)')
  .requiredOption('--doc <sparse>', 'Document sparse vector (term frequencies)')
  .requiredOption('--doc-len <number>', 'Document length')
  .requiredOption('--avg-doc-len <number>', 'Average document length')
  .option('--k1 <number>', 'Term frequency saturation', '1.2')
  .option('--b <number>', 'Length normalization', '0.75')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.bm25(client, options);
  });

sparse
  .command('top-k')
  .description('Keep only top-k elements by value')
  .requiredOption('-s, --sparse <vector>', 'Sparse vector')
  .requiredOption('-k, --k <number>', 'Number of elements to keep')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.topK(client, options);
  });

sparse
  .command('prune')
  .description('Remove elements below threshold')
  .requiredOption('-s, --sparse <vector>', 'Sparse vector')
  .requiredOption('--threshold <number>', 'Minimum absolute value threshold')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.prune(client, options);
  });

sparse
  .command('dense-to-sparse')
  .description('Convert dense vector to sparse')
  .requiredOption('-d, --dense <array>', 'Dense vector as JSON array')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.denseToSparse(client, options);
  });

sparse
  .command('sparse-to-dense <sparse>')
  .description('Convert sparse vector to dense')
  .action(async (sparseVec) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.sparseToDense(client, sparseVec);
  });

sparse
  .command('info <sparse>')
  .description('Get sparse vector information')
  .action(async (sparseVec) => {
    const client = new RuVectorClient(program.opts().connection);
    await SparseCommands.info(client, sparseVec);
  });

sparse
  .command('help')
  .description('Show sparse vector help')
  .action(() => SparseCommands.showHelp());

// ============================================================================
// Hyperbolic Operations
// ============================================================================

const hyperbolic = program.command('hyperbolic').description('Hyperbolic geometry operations');

hyperbolic
  .command('poincare-distance')
  .description('Compute Poincare ball distance')
  .requiredOption('-a, --a <vector>', 'First vector as JSON array')
  .requiredOption('-b, --b <vector>', 'Second vector as JSON array')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.poincareDistance(client, options);
  });

hyperbolic
  .command('lorentz-distance')
  .description('Compute Lorentz/hyperboloid distance')
  .requiredOption('-a, --a <vector>', 'First vector as JSON array')
  .requiredOption('-b, --b <vector>', 'Second vector as JSON array')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.lorentzDistance(client, options);
  });

hyperbolic
  .command('mobius-add')
  .description('Perform Mobius addition in Poincare ball')
  .requiredOption('-a, --a <vector>', 'First vector as JSON array')
  .requiredOption('-b, --b <vector>', 'Second vector as JSON array')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.mobiusAdd(client, options);
  });

hyperbolic
  .command('exp-map')
  .description('Exponential map: tangent space to manifold')
  .requiredOption('--base <vector>', 'Base point on manifold')
  .requiredOption('--tangent <vector>', 'Tangent vector at base')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.expMap(client, options);
  });

hyperbolic
  .command('log-map')
  .description('Logarithmic map: manifold to tangent space')
  .requiredOption('--base <vector>', 'Base point on manifold')
  .requiredOption('--target <vector>', 'Target point on manifold')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.logMap(client, options);
  });

hyperbolic
  .command('poincare-to-lorentz')
  .description('Convert Poincare to Lorentz coordinates')
  .requiredOption('--vector <array>', 'Poincare vector')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.poincareToLorentz(client, options);
  });

hyperbolic
  .command('lorentz-to-poincare')
  .description('Convert Lorentz to Poincare coordinates')
  .requiredOption('--vector <array>', 'Lorentz vector')
  .option('--curvature <number>', 'Curvature (negative)', '-1.0')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.lorentzToPoincare(client, options);
  });

hyperbolic
  .command('minkowski-dot')
  .description('Compute Minkowski inner product')
  .requiredOption('-a, --a <vector>', 'First vector')
  .requiredOption('-b, --b <vector>', 'Second vector')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await HyperbolicCommands.minkowskiDot(client, options.a, options.b);
  });

hyperbolic
  .command('help')
  .description('Show hyperbolic geometry help')
  .action(() => HyperbolicCommands.showHelp());

// ============================================================================
// Routing/Agent Operations
// ============================================================================

const routing = program.command('routing').description('Tiny Dancer agent routing');

routing
  .command('register')
  .description('Register a new agent')
  .requiredOption('--name <name>', 'Agent name')
  .requiredOption('--type <type>', 'Agent type (llm, embedding, specialized)')
  .requiredOption('--capabilities <list>', 'Capabilities (comma-separated)')
  .requiredOption('--cost <number>', 'Cost per request in dollars')
  .requiredOption('--latency <number>', 'Average latency in ms')
  .requiredOption('--quality <number>', 'Quality score (0-1)')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.registerAgent(client, options);
  });

routing
  .command('register-full')
  .description('Register agent with full JSON config')
  .requiredOption('--config <json>', 'Full agent configuration as JSON')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.registerAgentFull(client, options);
  });

routing
  .command('update')
  .description('Update agent metrics after a request')
  .requiredOption('--name <name>', 'Agent name')
  .requiredOption('--latency <number>', 'Observed latency in ms')
  .requiredOption('--success <boolean>', 'Whether request succeeded')
  .option('--quality <number>', 'Quality score for this request')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.updateMetrics(client, {
      ...options,
      success: options.success === 'true',
    });
  });

routing
  .command('remove <name>')
  .description('Remove an agent')
  .action(async (name) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.removeAgent(client, name);
  });

routing
  .command('set-active <name> <active>')
  .description('Enable or disable an agent')
  .action(async (name, active) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.setActive(client, name, active === 'true');
  });

routing
  .command('route')
  .description('Route a request to the best agent')
  .requiredOption('--embedding <array>', 'Request embedding as JSON array')
  .option('--optimize-for <target>', 'Optimization target (cost, latency, quality, balanced)', 'balanced')
  .option('--constraints <json>', 'Routing constraints as JSON')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.route(client, options);
  });

routing
  .command('list')
  .description('List all registered agents')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.listAgents(client);
  });

routing
  .command('get <name>')
  .description('Get detailed agent information')
  .action(async (name) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.getAgent(client, name);
  });

routing
  .command('find')
  .description('Find agents by capability')
  .requiredOption('--capability <name>', 'Capability to search for')
  .option('--limit <number>', 'Maximum results', '10')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.findByCapability(client, options);
  });

routing
  .command('stats')
  .description('Get routing statistics')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.stats(client);
  });

routing
  .command('clear')
  .description('Clear all agents')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    await RoutingCommands.clearAgents(client);
  });

routing
  .command('help')
  .description('Show routing help')
  .action(() => RoutingCommands.showHelp());

// ============================================================================
// Quantization Operations
// ============================================================================

const quantization = program.command('quantization').description('Vector quantization operations');
quantization.alias('quant');

quantization
  .command('binary')
  .description('Binary quantize a vector (1-bit per dimension)')
  .requiredOption('--vector <array>', 'Vector as JSON array')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await QuantizationCommands.binaryQuantize(client, options);
  });

quantization
  .command('scalar')
  .description('Scalar quantize a vector (8-bit per dimension)')
  .requiredOption('--vector <array>', 'Vector as JSON array')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await QuantizationCommands.scalarQuantize(client, options);
  });

quantization
  .command('compare <vector>')
  .description('Compare all quantization methods on a vector')
  .action(async (vector) => {
    const client = new RuVectorClient(program.opts().connection);
    await QuantizationCommands.compare(client, vector);
  });

quantization
  .command('stats')
  .description('Show quantization statistics')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    await QuantizationCommands.stats(client);
  });

quantization
  .command('help')
  .description('Show quantization help')
  .action(() => QuantizationCommands.showHelp());

// ============================================================================
// Attention Operations
// ============================================================================

const attention = program.command('attention').description('Attention mechanism operations');

attention
  .command('compute')
  .description('Compute attention between vectors')
  .option('-q, --query <vector>', 'Query vector')
  .option('-k, --keys <vectors>', 'Key vectors (JSON array)')
  .option('-v, --values <vectors>', 'Value vectors (JSON array)')
  .option('-t, --type <type>', 'Attention type (scaled_dot, multi_head, flash)', 'scaled_dot')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await AttentionCommands.compute(client, options);
  });

attention
  .command('list-types')
  .description('List available attention types')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    await AttentionCommands.listTypes(client);
  });

// ============================================================================
// GNN Operations
// ============================================================================

const gnn = program.command('gnn').description('Graph Neural Network operations');

gnn
  .command('create <name>')
  .description('Create a GNN layer')
  .option('-t, --type <type>', 'GNN type (gcn, graphsage, gat, gin)', 'gcn')
  .option('-i, --input-dim <number>', 'Input dimensions', '384')
  .option('-o, --output-dim <number>', 'Output dimensions', '128')
  .action(async (name, options) => {
    const client = new RuVectorClient(program.opts().connection);
    await GnnCommands.create(client, name, options);
  });

gnn
  .command('forward <layer>')
  .description('Forward pass through GNN layer')
  .option('-f, --features <path>', 'Node features file')
  .option('-e, --edges <path>', 'Edge list file')
  .action(async (layer, options) => {
    const client = new RuVectorClient(program.opts().connection);
    await GnnCommands.forward(client, layer, options);
  });

// ============================================================================
// Graph Operations
// ============================================================================

const graph = program.command('graph').description('Graph and Cypher operations');

graph
  .command('create <name>')
  .description('Create a new graph')
  .action(async (name) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      await client.createGraph(name);
      console.log(chalk.green(`Graph '${name}' created successfully`));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('query <graphName> <cypher>')
  .description('Execute a Cypher query on a graph')
  .action(async (graphName, cypher) => {
    const client = new RuVectorClient(program.opts().connection);
    await GraphCommands.query(client, `${graphName}:${cypher}`);
  });

graph
  .command('create-node <graphName>')
  .description('Create a graph node')
  .option('-l, --labels <labels>', 'Node labels (comma-separated)')
  .option('-p, --properties <json>', 'Node properties as JSON')
  .action(async (graphName, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const labels = options.labels ? options.labels.split(',').map((l: string) => l.trim()) : [];
      const properties = options.properties ? JSON.parse(options.properties) : {};
      const nodeId = await client.addNode(graphName, labels, properties);
      console.log(chalk.green(`Node created with ID: ${nodeId}`));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('create-edge <graphName>')
  .description('Create a graph edge')
  .requiredOption('--from <id>', 'Source node ID')
  .requiredOption('--to <id>', 'Target node ID')
  .requiredOption('--type <type>', 'Edge type/label')
  .option('-p, --properties <json>', 'Edge properties as JSON', '{}')
  .action(async (graphName, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const properties = JSON.parse(options.properties);
      const edgeId = await client.addEdge(
        graphName,
        parseInt(options.from),
        parseInt(options.to),
        options.type,
        properties
      );
      console.log(chalk.green(`Edge created with ID: ${edgeId}`));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('shortest-path <graphName>')
  .description('Find shortest path between nodes')
  .requiredOption('--start <id>', 'Starting node ID')
  .requiredOption('--end <id>', 'Ending node ID')
  .option('--max-hops <number>', 'Maximum hops', '10')
  .action(async (graphName, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const path = await client.shortestPath(
        graphName,
        parseInt(options.start),
        parseInt(options.end),
        parseInt(options.maxHops)
      );
      console.log(chalk.bold.blue('\nShortest Path:'));
      console.log(`  ${chalk.green('Length:')} ${path.length}`);
      console.log(`  ${chalk.green('Cost:')} ${path.cost}`);
      console.log(`  ${chalk.green('Nodes:')} ${path.nodes.join(' -> ')}`);
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('stats <graphName>')
  .description('Get graph statistics')
  .action(async (graphName) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const stats = await client.graphStats(graphName);
      console.log(chalk.bold.blue(`\nGraph: ${stats.name}`));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Nodes:')} ${stats.node_count}`);
      console.log(`  ${chalk.green('Edges:')} ${stats.edge_count}`);
      console.log(`  ${chalk.green('Labels:')} ${stats.labels.join(', ') || 'none'}`);
      console.log(`  ${chalk.green('Edge Types:')} ${stats.edge_types.join(', ') || 'none'}`);
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('list')
  .description('List all graphs')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const graphs = await client.listGraphs();
      if (graphs.length === 0) {
        console.log(chalk.yellow('No graphs found'));
      } else {
        console.log(chalk.bold.blue(`\nGraphs (${graphs.length}):`));
        graphs.forEach(g => console.log(`  ${chalk.green('-')} ${g}`));
      }
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('delete <graphName>')
  .description('Delete a graph')
  .action(async (graphName) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      await client.deleteGraph(graphName);
      console.log(chalk.green(`Graph '${graphName}' deleted`));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

graph
  .command('traverse')
  .description('Traverse the graph (legacy)')
  .option('-s, --start <id>', 'Starting node ID')
  .option('-d, --depth <number>', 'Max traversal depth', '3')
  .option('-t, --type <type>', 'Traversal type (bfs, dfs)', 'bfs')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await GraphCommands.traverse(client, options);
  });

// ============================================================================
// Learning Operations
// ============================================================================

const learning = program.command('learning').description('Self-learning and ReasoningBank operations');

learning
  .command('enable <table>')
  .description('Enable learning for a table')
  .option('--max-trajectories <number>', 'Maximum trajectories to track', '1000')
  .option('--num-clusters <number>', 'Number of clusters for patterns', '10')
  .action(async (table, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const config = {
        max_trajectories: parseInt(options.maxTrajectories),
        num_clusters: parseInt(options.numClusters),
      };
      const result = await client.enableLearning(table, config);
      console.log(chalk.green(result));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

learning
  .command('stats <table>')
  .description('Get learning statistics for a table')
  .action(async (table) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const stats = await client.learningStats(table);
      console.log(chalk.bold.blue('\nLearning Statistics:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(chalk.bold('Trajectories:'));
      console.log(`  ${chalk.green('Total:')} ${stats.trajectories.total}`);
      console.log(`  ${chalk.green('With Feedback:')} ${stats.trajectories.with_feedback}`);
      console.log(`  ${chalk.green('Avg Latency:')} ${stats.trajectories.avg_latency_us}us`);
      console.log(`  ${chalk.green('Avg Precision:')} ${(stats.trajectories.avg_precision * 100).toFixed(1)}%`);
      console.log(`  ${chalk.green('Avg Recall:')} ${(stats.trajectories.avg_recall * 100).toFixed(1)}%`);
      console.log(chalk.bold('\nPatterns:'));
      console.log(`  ${chalk.green('Total:')} ${stats.patterns.total}`);
      console.log(`  ${chalk.green('Samples:')} ${stats.patterns.total_samples}`);
      console.log(`  ${chalk.green('Avg Confidence:')} ${(stats.patterns.avg_confidence * 100).toFixed(1)}%`);
      console.log(`  ${chalk.green('Total Usage:')} ${stats.patterns.total_usage}`);
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

learning
  .command('auto-tune <table>')
  .description('Auto-tune search parameters')
  .option('--optimize-for <target>', 'Optimization target (speed, accuracy, balanced)', 'balanced')
  .action(async (table, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const result = await client.autoTune(table, options.optimizeFor);
      console.log(chalk.bold.blue('\nAuto-Tune Results:'));
      console.log(JSON.stringify(result, null, 2));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

learning
  .command('extract-patterns <table>')
  .description('Extract patterns from trajectories')
  .option('--clusters <number>', 'Number of clusters', '10')
  .action(async (table, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const result = await client.extractPatterns(table, parseInt(options.clusters));
      console.log(chalk.green(result));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

learning
  .command('get-params <table>')
  .description('Get optimized search parameters for a query')
  .requiredOption('--query <vector>', 'Query vector as JSON array')
  .action(async (table, options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const queryVec = JSON.parse(options.query);
      const params = await client.getSearchParams(table, queryVec);
      console.log(chalk.bold.blue('\nOptimized Parameters:'));
      console.log(`  ${chalk.green('ef_search:')} ${params.ef_search}`);
      console.log(`  ${chalk.green('probes:')} ${params.probes}`);
      console.log(`  ${chalk.green('confidence:')} ${(params.confidence * 100).toFixed(1)}%`);
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

learning
  .command('clear <table>')
  .description('Clear all learning data for a table')
  .action(async (table) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const result = await client.clearLearning(table);
      console.log(chalk.green(result));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

learning
  .command('train')
  .description('Train from trajectories (legacy)')
  .option('-f, --file <path>', 'Trajectory data file')
  .option('-e, --epochs <number>', 'Training epochs', '10')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await LearningCommands.train(client, options);
  });

learning
  .command('predict')
  .description('Make a prediction (legacy)')
  .option('-i, --input <vector>', 'Input vector')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await LearningCommands.predict(client, options);
  });

// ============================================================================
// Benchmark Operations
// ============================================================================

const benchmark = program.command('bench').description('Benchmarking operations');

benchmark
  .command('run')
  .description('Run comprehensive benchmarks')
  .option('-t, --type <type>', 'Benchmark type (vector, attention, gnn, all)', 'all')
  .option('-s, --size <number>', 'Dataset size', '10000')
  .option('-d, --dim <number>', 'Vector dimensions', '384')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await BenchmarkCommands.run(client, options);
  });

benchmark
  .command('report')
  .description('Generate benchmark report')
  .option('-f, --format <type>', 'Output format (json, table, markdown)', 'table')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    await BenchmarkCommands.report(client, options);
  });

// ============================================================================
// Info & Utility Commands
// ============================================================================

program
  .command('info')
  .description('Show extension information and capabilities')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const info = await client.getExtensionInfo();

      console.log(chalk.bold.blue('\nRuVector PostgreSQL Extension'));
      console.log(chalk.gray('='.repeat(50)));
      console.log(`${chalk.green('Version:')} ${info.version}`);

      if (info.simd_info) {
        console.log(`${chalk.green('SIMD:')} ${info.simd_info}`);
      }

      console.log(`\n${chalk.green('Features:')}`);
      info.features.forEach(f => console.log(`  ${chalk.yellow('*')} ${f}`));

      // Get memory stats
      try {
        const memStats = await client.getMemoryStats();
        console.log(`\n${chalk.green('Memory Usage:')}`);
        console.log(`  Index Memory:       ${memStats.index_memory_mb.toFixed(2)} MB`);
        console.log(`  Vector Cache:       ${memStats.vector_cache_mb.toFixed(2)} MB`);
        console.log(`  Quantization:       ${memStats.quantization_tables_mb.toFixed(2)} MB`);
        console.log(`  ${chalk.bold('Total:')}              ${memStats.total_extension_mb.toFixed(2)} MB`);
      } catch {
        // Memory stats may not be available
      }

      console.log();
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

program
  .command('extension')
  .description('Install/upgrade RuVector extension in existing PostgreSQL')
  .option('--upgrade', 'Upgrade existing installation')
  .action(async (options) => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      await client.installExtension(options.upgrade);
      console.log(chalk.green('RuVector extension installed successfully!'));
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

program
  .command('memory')
  .description('Show memory statistics')
  .action(async () => {
    const client = new RuVectorClient(program.opts().connection);
    try {
      await client.connect();
      const stats = await client.getMemoryStats();

      console.log(chalk.bold.blue('\nMemory Statistics:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Index Memory:')}       ${stats.index_memory_mb.toFixed(2)} MB`);
      console.log(`  ${chalk.green('Vector Cache:')}       ${stats.vector_cache_mb.toFixed(2)} MB`);
      console.log(`  ${chalk.green('Quantization:')}       ${stats.quantization_tables_mb.toFixed(2)} MB`);
      console.log(`  ${chalk.bold.green('Total:')}              ${stats.total_extension_mb.toFixed(2)} MB`);
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
    } finally {
      await client.disconnect();
    }
  });

// ============================================================================
// Installation & Server Management
// ============================================================================

program
  .command('install')
  .description('Install RuVector PostgreSQL (Docker or native with full dependency installation)')
  .option('-m, --method <type>', 'Installation method: docker, native, auto', 'auto')
  .option('-p, --port <number>', 'PostgreSQL port', '5432')
  .option('-u, --user <name>', 'Database user', 'ruvector')
  .option('--password <pass>', 'Database password', 'ruvector')
  .option('-d, --database <name>', 'Database name', 'ruvector')
  .option('--data-dir <path>', 'Persistent data directory')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .option('--version <version>', 'RuVector version', '0.2.5')
  .option('--pg-version <version>', 'PostgreSQL version for native install (14, 15, 16, 17)', '16')
  .option('--skip-postgres', 'Skip PostgreSQL installation (use existing)')
  .option('--skip-rust', 'Skip Rust installation (use existing)')
  .action(async (options) => {
    try {
      await InstallCommands.install({
        method: options.method,
        port: parseInt(options.port),
        user: options.user,
        password: options.password,
        database: options.database,
        dataDir: options.dataDir,
        name: options.name,
        version: options.version,
        pgVersion: options.pgVersion,
        skipPostgres: options.skipPostgres,
        skipRust: options.skipRust,
      });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program
  .command('uninstall')
  .description('Uninstall RuVector PostgreSQL')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .option('--remove-data', 'Also remove data volumes')
  .action(async (options) => {
    try {
      await InstallCommands.uninstall({
        name: options.name,
        removeData: options.removeData,
      });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program
  .command('status')
  .description('Show RuVector PostgreSQL installation status')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .action(async (options) => {
    try {
      await InstallCommands.printStatus({ name: options.name });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program
  .command('start')
  .description('Start RuVector PostgreSQL')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .action(async (options) => {
    try {
      await InstallCommands.start({ name: options.name });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program
  .command('stop')
  .description('Stop RuVector PostgreSQL')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .action(async (options) => {
    try {
      await InstallCommands.stop({ name: options.name });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program
  .command('logs')
  .description('Show RuVector PostgreSQL logs')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .option('-f, --follow', 'Follow log output')
  .option('-n, --tail <lines>', 'Number of lines to show', '100')
  .action(async (options) => {
    try {
      await InstallCommands.logs({
        name: options.name,
        follow: options.follow,
        tail: parseInt(options.tail),
      });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program
  .command('psql [command]')
  .description('Connect to RuVector PostgreSQL or execute SQL')
  .option('--name <name>', 'Container name', 'ruvector-postgres')
  .action(async (command, options) => {
    try {
      await InstallCommands.psql({
        name: options.name,
        command: command,
      });
    } catch (err) {
      console.error(chalk.red('Error:'), (err as Error).message);
      process.exit(1);
    }
  });

program.parse();
