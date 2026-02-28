/**
 * Sparse Vector Commands
 * CLI commands for sparse vector operations including BM25, sparsification, and distance calculations
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import { readFileSync } from 'fs';
import type { RuVectorClient } from '../client.js';

export interface SparseCreateOptions {
  indices: string;
  values: string;
  dim: string;
}

export interface SparseDistanceOptions {
  a: string;
  b: string;
  metric: 'dot' | 'cosine' | 'euclidean' | 'manhattan';
}

export interface SparseBM25Options {
  query: string;
  doc: string;
  docLen: string;
  avgDocLen: string;
  k1?: string;
  b?: string;
}

export interface SparseTopKOptions {
  sparse: string;
  k: string;
}

export interface SparsePruneOptions {
  sparse: string;
  threshold: string;
}

export interface DenseToSparseOptions {
  dense: string;
}

export class SparseCommands {
  static async create(
    client: RuVectorClient,
    options: SparseCreateOptions
  ): Promise<void> {
    const spinner = ora('Creating sparse vector...').start();

    try {
      await client.connect();

      const indices = JSON.parse(options.indices);
      const values = JSON.parse(options.values);
      const dim = parseInt(options.dim);

      const result = await client.createSparseVector(indices, values, dim);

      spinner.succeed(chalk.green('Sparse vector created successfully'));

      console.log(chalk.bold.blue('\nSparse Vector Details:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Indices:')} ${indices.length}`);
      console.log(`  ${chalk.green('Non-zero elements:')} ${values.length}`);
      console.log(`  ${chalk.green('Dimension:')} ${dim}`);
      console.log(`  ${chalk.green('Sparsity:')} ${((1 - values.length / dim) * 100).toFixed(2)}%`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to create sparse vector'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async distance(
    client: RuVectorClient,
    options: SparseDistanceOptions
  ): Promise<void> {
    const spinner = ora(`Computing sparse ${options.metric} distance...`).start();

    try {
      await client.connect();

      const result = await client.sparseDistance(options.a, options.b, options.metric);

      spinner.succeed(chalk.green(`Sparse ${options.metric} distance computed`));

      console.log(chalk.bold.blue('\nDistance Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Metric:')} ${options.metric}`);
      console.log(`  ${chalk.green('Distance:')} ${result.toFixed(6)}`);
    } catch (err) {
      spinner.fail(chalk.red('Distance computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async bm25(
    client: RuVectorClient,
    options: SparseBM25Options
  ): Promise<void> {
    const spinner = ora('Computing BM25 score...').start();

    try {
      await client.connect();

      const k1 = options.k1 ? parseFloat(options.k1) : 1.2;
      const b = options.b ? parseFloat(options.b) : 0.75;

      const score = await client.sparseBM25(
        options.query,
        options.doc,
        parseFloat(options.docLen),
        parseFloat(options.avgDocLen),
        k1,
        b
      );

      spinner.succeed(chalk.green('BM25 score computed'));

      console.log(chalk.bold.blue('\nBM25 Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Score:')} ${score.toFixed(6)}`);
      console.log(`  ${chalk.green('k1:')} ${k1}`);
      console.log(`  ${chalk.green('b:')} ${b}`);
      console.log(`  ${chalk.green('Document Length:')} ${options.docLen}`);
      console.log(`  ${chalk.green('Avg Doc Length:')} ${options.avgDocLen}`);
    } catch (err) {
      spinner.fail(chalk.red('BM25 computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async topK(
    client: RuVectorClient,
    options: SparseTopKOptions
  ): Promise<void> {
    const spinner = ora('Computing top-k sparse elements...').start();

    try {
      await client.connect();

      const result = await client.sparseTopK(options.sparse, parseInt(options.k));

      spinner.succeed(chalk.green('Top-k elements computed'));

      console.log(chalk.bold.blue('\nTop-K Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Original NNZ:')} ${result.originalNnz}`);
      console.log(`  ${chalk.green('After Top-K:')} ${result.newNnz}`);
      console.log(`  ${chalk.green('Sparse Vector:')} ${result.vector}`);
    } catch (err) {
      spinner.fail(chalk.red('Top-k computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async prune(
    client: RuVectorClient,
    options: SparsePruneOptions
  ): Promise<void> {
    const spinner = ora('Pruning sparse vector...').start();

    try {
      await client.connect();

      const result = await client.sparsePrune(
        options.sparse,
        parseFloat(options.threshold)
      );

      spinner.succeed(chalk.green('Sparse vector pruned'));

      console.log(chalk.bold.blue('\nPrune Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Threshold:')} ${options.threshold}`);
      console.log(`  ${chalk.green('Original NNZ:')} ${result.originalNnz ?? 'N/A'}`);
      console.log(`  ${chalk.green('After Pruning:')} ${result.newNnz ?? 'N/A'}`);
      console.log(`  ${chalk.green('Elements Removed:')} ${(result.originalNnz ?? 0) - (result.newNnz ?? 0)}`);
    } catch (err) {
      spinner.fail(chalk.red('Pruning failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async denseToSparse(
    client: RuVectorClient,
    options: DenseToSparseOptions
  ): Promise<void> {
    const spinner = ora('Converting dense to sparse...').start();

    try {
      await client.connect();

      const dense = JSON.parse(options.dense);
      const result = await client.denseToSparse(dense);

      spinner.succeed(chalk.green('Conversion completed'));

      console.log(chalk.bold.blue('\nConversion Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Dense Dimension:')} ${dense.length}`);
      console.log(`  ${chalk.green('Non-zero Elements:')} ${result.nnz}`);
      console.log(`  ${chalk.green('Sparsity:')} ${((1 - result.nnz / dense.length) * 100).toFixed(2)}%`);
      console.log(`  ${chalk.green('Sparse Vector:')} ${result.vector}`);
    } catch (err) {
      spinner.fail(chalk.red('Conversion failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async sparseToDense(
    client: RuVectorClient,
    sparse: string
  ): Promise<void> {
    const spinner = ora('Converting sparse to dense...').start();

    try {
      await client.connect();

      const result = await client.sparseToDense(sparse);

      spinner.succeed(chalk.green('Conversion completed'));

      console.log(chalk.bold.blue('\nConversion Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Dense Dimension:')} ${result.length}`);
      console.log(`  ${chalk.green('Non-zero Elements:')} ${result.filter((v: number) => v !== 0).length}`);

      // Show first 10 elements
      const preview = result.slice(0, 10).map((v: number) => v.toFixed(4)).join(', ');
      console.log(`  ${chalk.green('Preview:')} [${preview}${result.length > 10 ? ', ...' : ''}]`);
    } catch (err) {
      spinner.fail(chalk.red('Conversion failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async info(client: RuVectorClient, sparse: string): Promise<void> {
    const spinner = ora('Getting sparse vector info...').start();

    try {
      await client.connect();

      const info = await client.sparseInfo(sparse);

      spinner.stop();

      console.log(chalk.bold.blue('\nSparse Vector Info:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Dimension:')} ${info.dim}`);
      console.log(`  ${chalk.green('Non-zero Elements (NNZ):')} ${info.nnz}`);
      console.log(`  ${chalk.green('Sparsity:')} ${info.sparsity.toFixed(2)}%`);
      console.log(`  ${chalk.green('L2 Norm:')} ${info.norm.toFixed(6)}`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to get info'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showHelp(): void {
    console.log(chalk.bold.blue('\nSparse Vector Operations:'));
    console.log(chalk.gray('-'.repeat(60)));

    console.log(`
${chalk.yellow('Format:')}
  Sparse vectors use the format: '{index:value, index:value, ...}'
  Example: '{0:0.5, 10:0.3, 100:0.8}'

${chalk.yellow('Distance Metrics:')}
  ${chalk.green('dot')}       - Dot product (inner product)
  ${chalk.green('cosine')}    - Cosine similarity
  ${chalk.green('euclidean')} - L2 distance
  ${chalk.green('manhattan')} - L1 distance

${chalk.yellow('BM25 Scoring:')}
  Used for text search relevance ranking.
  Parameters:
    ${chalk.green('k1')} - Term frequency saturation (default: 1.2)
    ${chalk.green('b')}  - Length normalization (default: 0.75)

${chalk.yellow('Commands:')}
  ${chalk.green('sparse create')}         - Create sparse vector from indices/values
  ${chalk.green('sparse distance')}       - Compute distance between sparse vectors
  ${chalk.green('sparse bm25')}           - Compute BM25 relevance score
  ${chalk.green('sparse top-k')}          - Keep only top-k elements by value
  ${chalk.green('sparse prune')}          - Remove elements below threshold
  ${chalk.green('sparse dense-to-sparse')} - Convert dense to sparse
  ${chalk.green('sparse sparse-to-dense')} - Convert sparse to dense
  ${chalk.green('sparse info')}           - Get sparse vector statistics
`);
  }
}

export default SparseCommands;
