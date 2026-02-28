/**
 * GNN Commands
 * CLI commands for Graph Neural Network operations
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import { readFileSync } from 'fs';
import type { RuVectorClient } from '../client.js';

export interface GnnCreateOptions {
  type: 'gcn' | 'graphsage' | 'gat' | 'gin';
  inputDim: string;
  outputDim: string;
}

export interface GnnForwardOptions {
  features: string;
  edges: string;
}

export class GnnCommands {
  static async create(
    client: RuVectorClient,
    name: string,
    options: GnnCreateOptions
  ): Promise<void> {
    const spinner = ora(`Creating GNN layer '${name}'...`).start();

    try {
      await client.connect();

      await client.createGnnLayer(
        name,
        options.type,
        parseInt(options.inputDim),
        parseInt(options.outputDim)
      );

      spinner.succeed(chalk.green(`GNN layer '${name}' created successfully`));

      console.log(chalk.bold.blue('\nLayer Configuration:'));
      console.log(chalk.gray('─'.repeat(40)));
      console.log(`  ${chalk.green('Type:')} ${options.type.toUpperCase()}`);
      console.log(`  ${chalk.green('Input Dimensions:')} ${options.inputDim}`);
      console.log(`  ${chalk.green('Output Dimensions:')} ${options.outputDim}`);

      // Type-specific info
      const typeInfo: Record<string, string> = {
        gcn: 'Graph Convolutional Network - Spectral graph convolutions',
        graphsage: 'GraphSAGE - Inductive learning with neighborhood sampling',
        gat: 'Graph Attention Network - Attention-based message passing',
        gin: 'Graph Isomorphism Network - WL-test expressive power'
      };

      console.log(`\n  ${chalk.gray(typeInfo[options.type])}`);
    } catch (err) {
      spinner.fail(chalk.red('Failed to create GNN layer'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async forward(
    client: RuVectorClient,
    layer: string,
    options: GnnForwardOptions
  ): Promise<void> {
    const spinner = ora(`Running forward pass through '${layer}'...`).start();

    try {
      await client.connect();

      // Load features and edges from files
      const featuresContent = readFileSync(options.features, 'utf-8');
      const edgesContent = readFileSync(options.edges, 'utf-8');

      const features = JSON.parse(featuresContent) as number[][];
      const edges = JSON.parse(edgesContent) as [number, number][];

      // Extract src and dst from edges
      const src = edges.map(e => e[0]);
      const dst = edges.map(e => e[1]);
      const outDim = features[0]?.length || 64;

      const result = await client.gnnForward(layer as 'gcn' | 'sage', features, src, dst, outDim);

      spinner.succeed(chalk.green('Forward pass completed successfully'));

      console.log(chalk.bold.blue('\nGNN Output:'));
      console.log(chalk.gray('─'.repeat(40)));
      console.log(`  ${chalk.green('Nodes:')} ${result.length}`);
      console.log(`  ${chalk.green('Embedding Dim:')} ${result[0]?.length || 0}`);

      // Show sample embeddings
      console.log(chalk.bold.blue('\nSample Node Embeddings:'));

      const table = new Table({
        head: [
          chalk.cyan('Node'),
          chalk.cyan('Embedding (first 8 dims)')
        ],
        colWidths: [8, 60]
      });

      for (let i = 0; i < Math.min(5, result.length); i++) {
        const emb = result[i];
        table.push([
          `${i}`,
          `[${emb.slice(0, 8).map((v: number) => v.toFixed(4)).join(', ')}${emb.length > 8 ? '...' : ''}]`
        ]);
      }

      console.log(table.toString());

      if (result.length > 5) {
        console.log(chalk.gray(`  ... and ${result.length - 5} more nodes`));
      }
    } catch (err) {
      spinner.fail(chalk.red('Forward pass failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async listTypes(): Promise<void> {
    console.log(chalk.bold.blue('\nAvailable GNN Layer Types:'));
    console.log(chalk.gray('─'.repeat(50)));

    const types = [
      {
        name: 'GCN',
        desc: 'Graph Convolutional Network',
        details: 'Spectral graph convolutions using Chebyshev polynomials'
      },
      {
        name: 'GraphSAGE',
        desc: 'Sample and Aggregate',
        details: 'Inductive learning with neighborhood sampling and aggregation'
      },
      {
        name: 'GAT',
        desc: 'Graph Attention Network',
        details: 'Attention-weighted message passing between nodes'
      },
      {
        name: 'GIN',
        desc: 'Graph Isomorphism Network',
        details: 'Provably as powerful as WL-test for graph isomorphism'
      }
    ];

    for (const type of types) {
      console.log(`\n  ${chalk.yellow(type.name)} - ${type.desc}`);
      console.log(`    ${chalk.gray(type.details)}`);
    }

    console.log();
  }
}

export default GnnCommands;
