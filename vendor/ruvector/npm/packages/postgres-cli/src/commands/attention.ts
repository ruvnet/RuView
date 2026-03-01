/**
 * Attention Commands
 * CLI commands for attention mechanism operations
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import type { RuVectorClient } from '../client.js';

export interface AttentionComputeOptions {
  query: string;
  keys: string;
  values: string;
  type: 'scaled_dot' | 'multi_head' | 'flash';
}

export class AttentionCommands {
  static async compute(
    client: RuVectorClient,
    options: AttentionComputeOptions
  ): Promise<void> {
    const spinner = ora('Computing attention...').start();

    try {
      await client.connect();

      const query = JSON.parse(options.query) as number[];
      const keys = JSON.parse(options.keys) as number[][];
      const values = JSON.parse(options.values) as number[][];

      const result = await client.computeAttention(query, keys, values, options.type);

      spinner.succeed(chalk.green('Attention computed successfully'));

      console.log(chalk.bold.blue('\nAttention Output:'));
      console.log(chalk.gray('─'.repeat(40)));

      // Display output vector
      console.log(`${chalk.green('Output Vector:')} [${result.output.slice(0, 8).map(v => v.toFixed(4)).join(', ')}${result.output.length > 8 ? '...' : ''}]`);
      console.log(`${chalk.gray('Dimensions:')} ${result.output.length}`);

      // Display attention weights if available
      if (result.weights) {
        console.log(chalk.bold.blue('\nAttention Weights:'));
        const table = new Table({
          head: keys.map((_, i) => chalk.cyan(`K${i}`)),
        });

        for (let i = 0; i < Math.min(result.weights.length, 5); i++) {
          table.push(result.weights[i].slice(0, keys.length).map(w => w.toFixed(4)));
        }

        console.log(table.toString());
      }
    } catch (err) {
      spinner.fail(chalk.red('Failed to compute attention'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async listTypes(client: RuVectorClient): Promise<void> {
    const spinner = ora('Fetching attention types...').start();

    try {
      await client.connect();

      const types = await client.listAttentionTypes();

      spinner.stop();

      console.log(chalk.bold.blue('\nAvailable Attention Mechanisms:'));
      console.log(chalk.gray('─'.repeat(40)));

      // Group by category
      const categories = {
        'Core': ['scaled_dot_product_attention', 'multi_head_attention', 'flash_attention'],
        'Sparse': ['sparse_attention', 'local_attention', 'strided_attention', 'random_attention', 'longformer_attention'],
        'Memory': ['memory_attention', 'compressive_attention', 'memory_compressed_attention'],
        'Cross-Modal': ['cross_attention', 'cross_modal_attention', 'multimodal_attention'],
        'Efficient': ['linear_attention', 'performer_attention', 'reformer_attention', 'synthesizer_attention'],
        'Positional': ['relative_attention', 'rotary_attention', 'alibi_attention', 'rope_attention'],
        'Graph': ['graph_attention', 'gat_attention', 'sparse_graph_attention'],
        'Advanced': ['self_attention', 'causal_attention', 'bidirectional_attention', 'grouped_query_attention'],
      };

      for (const [category, items] of Object.entries(categories)) {
        const available = items.filter(t => types.includes(t));
        if (available.length > 0) {
          console.log(`\n${chalk.yellow(category)}:`);
          for (const item of available) {
            console.log(`  ${chalk.green('✓')} ${item}`);
          }
        }
      }

      // Show any types not in categories
      const categorized = Object.values(categories).flat();
      const uncategorized = types.filter(t => !categorized.includes(t));
      if (uncategorized.length > 0) {
        console.log(`\n${chalk.yellow('Other')}:`);
        for (const item of uncategorized) {
          console.log(`  ${chalk.green('✓')} ${item}`);
        }
      }

      console.log(`\n${chalk.gray(`Total: ${types.length} attention mechanisms`)}`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to list attention types'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }
}

export default AttentionCommands;
