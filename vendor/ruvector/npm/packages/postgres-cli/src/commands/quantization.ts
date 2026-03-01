/**
 * Quantization Commands
 * CLI commands for vector quantization operations (binary, scalar, product)
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import type { RuVectorClient } from '../client.js';

export interface BinaryQuantizeOptions {
  vector: string;
}

export interface ScalarQuantizeOptions {
  vector: string;
}

export interface QuantizedSearchOptions {
  table: string;
  query: string;
  topK?: string;
  quantType?: 'binary' | 'scalar';
}

export class QuantizationCommands {
  static async binaryQuantize(
    client: RuVectorClient,
    options: BinaryQuantizeOptions
  ): Promise<void> {
    const spinner = ora('Binary quantizing vector...').start();

    try {
      await client.connect();

      const vector = JSON.parse(options.vector);
      const result = await client.binaryQuantize(vector);

      spinner.succeed(chalk.green('Binary quantization completed'));

      console.log(chalk.bold.blue('\nBinary Quantization Result:'));
      console.log(chalk.gray('-'.repeat(50)));
      console.log(`  ${chalk.green('Original Dimension:')} ${vector.length}`);
      console.log(`  ${chalk.green('Quantized Bytes:')} ${result.length}`);
      console.log(`  ${chalk.green('Compression Ratio:')} ${(vector.length * 4 / result.length).toFixed(1)}x`);
      console.log(`  ${chalk.green('Memory Savings:')} ${((1 - result.length / (vector.length * 4)) * 100).toFixed(1)}%`);

      // Show first few bytes as hex
      const hexPreview = result.slice(0, 16).map((b: number) => b.toString(16).padStart(2, '0')).join(' ');
      console.log(`  ${chalk.green('Preview (hex):')} ${hexPreview}${result.length > 16 ? '...' : ''}`);
    } catch (err) {
      spinner.fail(chalk.red('Binary quantization failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async scalarQuantize(
    client: RuVectorClient,
    options: ScalarQuantizeOptions
  ): Promise<void> {
    const spinner = ora('Scalar quantizing vector (SQ8)...').start();

    try {
      await client.connect();

      const vector = JSON.parse(options.vector);
      const result = await client.scalarQuantize(vector);

      spinner.succeed(chalk.green('Scalar quantization completed'));

      console.log(chalk.bold.blue('\nScalar Quantization (SQ8) Result:'));
      console.log(chalk.gray('-'.repeat(50)));
      console.log(`  ${chalk.green('Original Dimension:')} ${vector.length}`);
      console.log(`  ${chalk.green('Quantized Elements:')} ${result.data.length}`);
      console.log(`  ${chalk.green('Scale Factor:')} ${result.scale.toFixed(6)}`);
      console.log(`  ${chalk.green('Offset:')} ${result.offset.toFixed(6)}`);
      console.log(`  ${chalk.green('Compression Ratio:')} 4x (32-bit to 8-bit)`);
      console.log(`  ${chalk.green('Memory Savings:')} 75%`);

      // Show reconstruction formula
      console.log(chalk.bold.blue('\nReconstruction:'));
      console.log(`  ${chalk.gray('original[i] = quantized[i] * scale + offset')}`);

      // Show preview
      const preview = result.data.slice(0, 10).join(', ');
      console.log(`  ${chalk.green('Quantized Preview:')} [${preview}${result.data.length > 10 ? ', ...' : ''}]`);
    } catch (err) {
      spinner.fail(chalk.red('Scalar quantization failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async stats(client: RuVectorClient): Promise<void> {
    const spinner = ora('Fetching quantization statistics...').start();

    try {
      await client.connect();

      const stats = await client.quantizationStats();

      spinner.stop();

      console.log(chalk.bold.blue('\nQuantization Statistics:'));
      console.log(chalk.gray('-'.repeat(50)));

      const table = new Table({
        head: [
          chalk.cyan('Type'),
          chalk.cyan('Bits/Dim'),
          chalk.cyan('Compression'),
          chalk.cyan('Accuracy Loss'),
          chalk.cyan('Speed Boost'),
        ],
        colWidths: [15, 12, 14, 15, 14],
      });

      table.push(
        ['Binary (BQ)', '1', '32x', '~20-30%', '~10-20x'],
        ['Scalar (SQ8)', '8', '4x', '~1-5%', '~2-4x'],
        ['Product (PQ)', 'Variable', '8-32x', '~5-15%', '~5-10x'],
      );

      console.log(table.toString());

      console.log(chalk.bold.blue('\nMemory Usage:'));
      console.log(`  ${chalk.green('Quantization Tables:')} ${stats.quantization_tables_mb.toFixed(2)} MB`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to get stats'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async compare(
    client: RuVectorClient,
    vector: string
  ): Promise<void> {
    const spinner = ora('Comparing quantization methods...').start();

    try {
      await client.connect();

      const vec = JSON.parse(vector);
      const dim = vec.length;

      // Get all quantization results
      const binary = await client.binaryQuantize(vec);
      const scalar = await client.scalarQuantize(vec);

      spinner.stop();

      console.log(chalk.bold.blue('\nQuantization Comparison:'));
      console.log(chalk.gray('-'.repeat(60)));
      console.log(`  ${chalk.green('Original Vector:')} ${dim} dimensions, ${dim * 4} bytes`);

      const table = new Table({
        head: [
          chalk.cyan('Method'),
          chalk.cyan('Size'),
          chalk.cyan('Compression'),
          chalk.cyan('Type'),
        ],
        colWidths: [18, 15, 15, 20],
      });

      table.push(
        ['Original (f32)', `${dim * 4} bytes`, '1x', '32-bit float'],
        ['Binary (BQ)', `${binary.length} bytes`, `${(dim * 4 / binary.length).toFixed(1)}x`, '1-bit per dim'],
        ['Scalar (SQ8)', `${scalar.data.length + 8} bytes`, `${(dim * 4 / (scalar.data.length + 8)).toFixed(1)}x`, '8-bit + metadata'],
      );

      console.log(table.toString());

      console.log(chalk.bold.blue('\nTrade-offs:'));
      console.log(`  ${chalk.yellow('Binary:')} Best compression, lowest accuracy, fastest`);
      console.log(`  ${chalk.yellow('Scalar:')} Good balance of compression and accuracy`);
      console.log(`  ${chalk.yellow('Product:')} Variable, best for specific use cases`);
    } catch (err) {
      spinner.fail(chalk.red('Comparison failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showHelp(): void {
    console.log(chalk.bold.blue('\nVector Quantization:'));
    console.log(chalk.gray('-'.repeat(60)));

    console.log(`
${chalk.yellow('Overview:')}
  Quantization reduces vector storage size and speeds up search
  by representing vectors with fewer bits per dimension.

${chalk.yellow('Quantization Types:')}

  ${chalk.green('Binary Quantization (BQ)')}
  - Converts each dimension to 1 bit (sign)
  - 32x memory reduction
  - 10-20x search speedup
  - ~20-30% accuracy loss
  - Best for: Large-scale approximate search

  ${chalk.green('Scalar Quantization (SQ8)')}
  - Converts 32-bit floats to 8-bit integers
  - 4x memory reduction
  - 2-4x search speedup
  - ~1-5% accuracy loss
  - Best for: Balanced accuracy/efficiency

  ${chalk.green('Product Quantization (PQ)')}
  - Splits vector into subvectors, each quantized separately
  - 8-32x memory reduction
  - 5-10x search speedup
  - ~5-15% accuracy loss
  - Best for: Medium-scale with accuracy needs

${chalk.yellow('Commands:')}
  ${chalk.green('quantization binary')}   - Binary quantize a vector
  ${chalk.green('quantization scalar')}   - Scalar quantize (SQ8)
  ${chalk.green('quantization compare')}  - Compare all methods
  ${chalk.green('quantization stats')}    - View quantization statistics

${chalk.yellow('When to Use:')}
  - Dataset > 1M vectors: Consider BQ or PQ
  - Need < 5% accuracy loss: Use SQ8
  - Filtering important: Use BQ with re-ranking
  - Memory constrained: Use BQ or PQ
`);
  }
}

export default QuantizationCommands;
