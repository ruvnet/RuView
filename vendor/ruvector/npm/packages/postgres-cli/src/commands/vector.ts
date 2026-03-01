/**
 * Vector Commands
 * CLI commands for vector operations
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import { readFileSync } from 'fs';
import type { RuVectorClient } from '../client.js';

export interface VectorCreateOptions {
  dim: string;
  index: 'hnsw' | 'ivfflat';
}

export interface VectorInsertOptions {
  file?: string;
  text?: string;
}

export interface VectorSearchOptions {
  query?: string;
  text?: string;
  topK: string;
  metric: 'cosine' | 'l2' | 'ip';
}

export interface VectorDistanceOptions {
  a: string;
  b: string;
  metric: 'cosine' | 'l2' | 'ip';
}

export interface VectorNormalizeOptions {
  vector: string;
}

export class VectorCommands {
  static async distance(
    client: RuVectorClient,
    options: VectorDistanceOptions
  ): Promise<void> {
    const spinner = ora('Computing vector distance...').start();

    try {
      await client.connect();

      const a = JSON.parse(options.a);
      const b = JSON.parse(options.b);

      let distance: number;
      let metricName: string;

      switch (options.metric) {
        case 'l2':
          distance = await client.l2DistanceArr(a, b);
          metricName = 'L2 (Euclidean)';
          break;
        case 'ip':
          distance = await client.innerProductArr(a, b);
          metricName = 'Inner Product';
          break;
        case 'cosine':
        default:
          distance = await client.cosineDistanceArr(a, b);
          metricName = 'Cosine';
          break;
      }

      spinner.succeed(chalk.green('Distance computed'));

      console.log(chalk.bold.blue('\nVector Distance:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Metric:')} ${metricName}`);
      console.log(`  ${chalk.green('Distance:')} ${distance.toFixed(6)}`);
      console.log(`  ${chalk.green('Dimension:')} ${a.length}`);

      // Additional context for cosine distance
      if (options.metric === 'cosine') {
        const similarity = 1 - distance;
        console.log(`  ${chalk.green('Similarity:')} ${similarity.toFixed(6)} (1 - distance)`);
      }
    } catch (err) {
      spinner.fail(chalk.red('Distance computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async normalize(
    client: RuVectorClient,
    options: VectorNormalizeOptions
  ): Promise<void> {
    const spinner = ora('Normalizing vector...').start();

    try {
      await client.connect();

      const vector = JSON.parse(options.vector);
      const normalized = await client.vectorNormalize(vector);

      spinner.succeed(chalk.green('Vector normalized'));

      console.log(chalk.bold.blue('\nNormalized Vector:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Original Dimension:')} ${vector.length}`);

      // Compute original norm for reference
      const originalNorm = Math.sqrt(vector.reduce((sum: number, v: number) => sum + v * v, 0));
      console.log(`  ${chalk.green('Original Norm:')} ${originalNorm.toFixed(6)}`);

      // Verify normalized norm is ~1
      const normalizedNorm = Math.sqrt(normalized.reduce((sum: number, v: number) => sum + v * v, 0));
      console.log(`  ${chalk.green('Normalized Norm:')} ${normalizedNorm.toFixed(6)}`);

      // Display vector (truncated if too long)
      if (normalized.length <= 10) {
        console.log(`  ${chalk.green('Result:')} [${normalized.map((v: number) => v.toFixed(4)).join(', ')}]`);
      } else {
        const first5 = normalized.slice(0, 5).map((v: number) => v.toFixed(4)).join(', ');
        const last3 = normalized.slice(-3).map((v: number) => v.toFixed(4)).join(', ');
        console.log(`  ${chalk.green('Result:')} [${first5}, ..., ${last3}]`);
      }
    } catch (err) {
      spinner.fail(chalk.red('Normalization failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async create(
    client: RuVectorClient,
    name: string,
    options: VectorCreateOptions
  ): Promise<void> {
    const spinner = ora(`Creating vector table '${name}'...`).start();

    try {
      await client.connect();
      await client.createVectorTable(
        name,
        parseInt(options.dim),
        options.index
      );

      spinner.succeed(chalk.green(`Vector table '${name}' created successfully`));
      console.log(`  ${chalk.gray('Dimensions:')} ${options.dim}`);
      console.log(`  ${chalk.gray('Index Type:')} ${options.index.toUpperCase()}`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to create vector table'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async insert(
    client: RuVectorClient,
    table: string,
    options: VectorInsertOptions
  ): Promise<void> {
    const spinner = ora(`Inserting vectors into '${table}'...`).start();

    try {
      await client.connect();

      let vectors: { vector: number[]; metadata?: Record<string, unknown> }[] = [];

      if (options.file) {
        const content = readFileSync(options.file, 'utf-8');
        const data = JSON.parse(content);
        vectors = Array.isArray(data) ? data : [data];
      } else if (options.text) {
        // For text, we'd need an embedding model
        // For now, just show a placeholder
        console.log(chalk.yellow('Note: Text embedding requires an embedding model'));
        console.log(chalk.gray('Using placeholder embedding...'));
        vectors = [{
          vector: Array(384).fill(0).map(() => Math.random()),
          metadata: { text: options.text }
        }];
      }

      let inserted = 0;
      for (const item of vectors) {
        await client.insertVector(table, item.vector, item.metadata);
        inserted++;
      }

      spinner.succeed(chalk.green(`Inserted ${inserted} vector(s) into '${table}'`));
    } catch (err) {
      spinner.fail(chalk.red('Failed to insert vectors'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async search(
    client: RuVectorClient,
    table: string,
    options: VectorSearchOptions
  ): Promise<void> {
    const spinner = ora(`Searching vectors in '${table}'...`).start();

    try {
      await client.connect();

      let queryVector: number[];

      if (options.query) {
        queryVector = JSON.parse(options.query);
      } else if (options.text) {
        console.log(chalk.yellow('Note: Text embedding requires an embedding model'));
        console.log(chalk.gray('Using placeholder embedding...'));
        queryVector = Array(384).fill(0).map(() => Math.random());
      } else {
        throw new Error('Either --query or --text is required');
      }

      const results = await client.searchVectors(
        table,
        queryVector,
        parseInt(options.topK),
        options.metric
      );

      spinner.stop();

      if (results.length === 0) {
        console.log(chalk.yellow('No results found'));
        return;
      }

      const resultTable = new Table({
        head: [
          chalk.cyan('ID'),
          chalk.cyan('Distance'),
          chalk.cyan('Metadata')
        ],
        colWidths: [10, 15, 50]
      });

      for (const result of results) {
        resultTable.push([
          String(result.id),
          result.distance.toFixed(6),
          result.metadata ? JSON.stringify(result.metadata).slice(0, 45) + '...' : '-'
        ]);
      }

      console.log(chalk.bold.blue(`\nSearch Results (${results.length} matches)`));
      console.log(resultTable.toString());
    } catch (err) {
      spinner.fail(chalk.red('Search failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }
}

export default VectorCommands;
