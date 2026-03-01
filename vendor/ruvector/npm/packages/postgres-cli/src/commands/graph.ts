/**
 * Graph Commands
 * CLI commands for graph operations and Cypher queries
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import type { RuVectorClient } from '../client.js';

export interface CreateNodeOptions {
  labels: string;
  properties: string;
}

export interface TraverseOptions {
  start: string;
  depth: string;
  type: 'bfs' | 'dfs';
}

export class GraphCommands {
  static async query(
    client: RuVectorClient,
    cypher: string
  ): Promise<void> {
    const spinner = ora('Executing Cypher query...').start();

    try {
      await client.connect();

      const results = await client.cypherQuery('default', cypher);

      spinner.stop();

      if (!results || results.length === 0) {
        console.log(chalk.yellow('Query executed successfully, no results returned'));
        return;
      }

      console.log(chalk.bold.blue(`\nQuery Results (${results.length} rows):`));
      console.log(chalk.gray('─'.repeat(60)));

      // Auto-detect columns from first result
      const firstRow = results[0] as Record<string, unknown>;
      const columns = Object.keys(firstRow);

      const table = new Table({
        head: columns.map(c => chalk.cyan(c)),
        colWidths: columns.map(() => Math.floor(60 / columns.length))
      });

      for (const row of results.slice(0, 20)) {
        const r = row as Record<string, unknown>;
        table.push(columns.map(c => {
          const val = r[c];
          if (typeof val === 'object') {
            return JSON.stringify(val).slice(0, 20) + '...';
          }
          return String(val).slice(0, 20);
        }));
      }

      console.log(table.toString());

      if (results.length > 20) {
        console.log(chalk.gray(`... and ${results.length - 20} more rows`));
      }
    } catch (err) {
      spinner.fail(chalk.red('Query failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async createNode(
    client: RuVectorClient,
    options: CreateNodeOptions
  ): Promise<void> {
    const spinner = ora('Creating graph node...').start();

    try {
      await client.connect();

      const labels = options.labels.split(',').map(l => l.trim());
      const properties = JSON.parse(options.properties);

      const nodeId = await client.addNode('default', labels, properties);

      spinner.succeed(chalk.green('Node created successfully'));

      console.log(chalk.bold.blue('\nNode Details:'));
      console.log(chalk.gray('─'.repeat(40)));
      console.log(`  ${chalk.green('ID:')} ${nodeId}`);
      console.log(`  ${chalk.green('Labels:')} ${labels.join(', ')}`);
      console.log(`  ${chalk.green('Properties:')}`);

      for (const [key, value] of Object.entries(properties)) {
        console.log(`    ${chalk.gray(key + ':')} ${JSON.stringify(value)}`);
      }
    } catch (err) {
      spinner.fail(chalk.red('Failed to create node'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async traverse(
    client: RuVectorClient,
    options: TraverseOptions
  ): Promise<void> {
    const spinner = ora(`Traversing graph from node ${options.start}...`).start();

    try {
      await client.connect();

      // Use Cypher query to find neighbors
      const cypherQuery = `MATCH (n)-[*1..${options.depth}]-(m) WHERE id(n) = ${options.start} RETURN m`;
      const results = await client.cypherQuery('default', cypherQuery);

      spinner.succeed(chalk.green('Traversal completed'));

      console.log(chalk.bold.blue('\nTraversal Results:'));
      console.log(chalk.gray('─'.repeat(50)));
      console.log(`  ${chalk.green('Algorithm:')} ${options.type.toUpperCase()}`);
      console.log(`  ${chalk.green('Max Depth:')} ${options.depth}`);
      console.log(`  ${chalk.green('Nodes Found:')} ${results.length}`);

      // Show nodes found
      if (results.length > 0) {
        console.log(chalk.bold.blue('\nFound Nodes:'));

        const nodeTable = new Table({
          head: [chalk.cyan('Node')],
          colWidths: [60]
        });

        for (const row of results.slice(0, 10)) {
          nodeTable.push([
            JSON.stringify(row).slice(0, 55) + '...'
          ]);
        }

        console.log(nodeTable.toString());

        if (results.length > 10) {
          console.log(chalk.gray(`... and ${results.length - 10} more nodes`));
        }
      }
    } catch (err) {
      spinner.fail(chalk.red('Traversal failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showSyntax(): void {
    console.log(chalk.bold.blue('\nCypher Query Syntax:'));
    console.log(chalk.gray('─'.repeat(60)));

    const examples = [
      { query: 'MATCH (n) RETURN n LIMIT 10', desc: 'Return first 10 nodes' },
      { query: 'MATCH (n:Person) RETURN n', desc: 'Find all Person nodes' },
      { query: 'MATCH (a)-[r]->(b) RETURN a,r,b', desc: 'Find relationships' },
      { query: "MATCH (n {name: 'Alice'}) RETURN n", desc: 'Find by property' },
      { query: 'MATCH p=(a)-[*1..3]->(b) RETURN p', desc: 'Variable-length path' },
      { query: "CREATE (n:Person {name: 'Bob'}) RETURN n", desc: 'Create a node' },
    ];

    for (const ex of examples) {
      console.log(`\n  ${chalk.yellow(ex.desc)}`);
      console.log(`  ${chalk.green('>')} ${ex.query}`);
    }

    console.log();
  }
}

export default GraphCommands;
