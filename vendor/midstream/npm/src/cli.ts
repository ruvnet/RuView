#!/usr/bin/env node
/**
 * MidStream CLI - Command-line interface for Lean Agentic Learning System
 */

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import inquirer from 'inquirer';
import { MidStreamAgent } from './agent.js';
import { WebSocketStreamServer, SSEStreamServer } from './streaming.js';
import { MidStreamMCPServer } from './mcp-server.js';
import * as fs from 'fs';
import * as path from 'path';

const program = new Command();

program
  .name('midstream')
  .description('MidStream - Real-time LLM streaming with Lean Agentic Learning')
  .version('0.1.0');

// ============================================================================
// Process command
// ============================================================================

program
  .command('process <message>')
  .description('Process a message through the agent')
  .option('-o, --output <file>', 'Output file for results')
  .action(async (message: string, options: any) => {
    const spinner = ora('Processing message...').start();

    try {
      const agent = new MidStreamAgent();
      const result = agent.processMessage(message);

      spinner.succeed('Message processed');

      console.log(chalk.bold('\nResult:'));
      console.log(JSON.stringify(result, null, 2));

      if (options.output) {
        fs.writeFileSync(options.output, JSON.stringify(result, null, 2));
        console.log(chalk.green(`\nSaved to ${options.output}`));
      }
    } catch (error) {
      spinner.fail('Processing failed');
      console.error(chalk.red(error instanceof Error ? error.message : String(error)));
      process.exit(1);
    }
  });

// ============================================================================
// Analyze command
// ============================================================================

program
  .command('analyze <file>')
  .description('Analyze a conversation from a JSON file')
  .option('-o, --output <file>', 'Output file for analysis results')
  .action(async (file: string, options: any) => {
    const spinner = ora('Analyzing conversation...').start();

    try {
      const data = JSON.parse(fs.readFileSync(file, 'utf-8'));
      const messages = Array.isArray(data) ? data : data.messages;

      const agent = new MidStreamAgent();
      const result = agent.analyzeConversation(messages);

      spinner.succeed('Analysis complete');

      console.log(chalk.bold('\nAnalysis Results:'));
      console.log(JSON.stringify(result, null, 2));

      if (options.output) {
        fs.writeFileSync(options.output, JSON.stringify(result, null, 2));
        console.log(chalk.green(`\nSaved to ${options.output}`));
      }
    } catch (error) {
      spinner.fail('Analysis failed');
      console.error(chalk.red(error instanceof Error ? error.message : String(error)));
      process.exit(1);
    }
  });

// ============================================================================
// Compare command
// ============================================================================

program
  .command('compare <file1> <file2>')
  .description('Compare two sequences using temporal analysis')
  .option('-a, --algorithm <algo>', 'Algorithm: dtw, lcs, edit, correlation', 'dtw')
  .action(async (file1: string, file2: string, options: any) => {
    const spinner = ora('Comparing sequences...').start();

    try {
      const seq1 = JSON.parse(fs.readFileSync(file1, 'utf-8'));
      const seq2 = JSON.parse(fs.readFileSync(file2, 'utf-8'));

      const agent = new MidStreamAgent();
      const similarity = agent.compareSequences(seq1, seq2, options.algorithm);

      spinner.succeed('Comparison complete');

      console.log(chalk.bold('\nComparison Results:'));
      console.log(`Algorithm: ${options.algorithm}`);
      console.log(`Similarity: ${similarity.toFixed(4)}`);

      if (similarity > 0.8) {
        console.log(chalk.green('Very similar sequences'));
      } else if (similarity > 0.6) {
        console.log(chalk.yellow('Moderately similar sequences'));
      } else {
        console.log(chalk.red('Different sequences'));
      }
    } catch (error) {
      spinner.fail('Comparison failed');
      console.error(chalk.red(error instanceof Error ? error.message : String(error)));
      process.exit(1);
    }
  });

// ============================================================================
// Serve command - Start streaming servers
// ============================================================================

program
  .command('serve')
  .description('Start WebSocket and SSE streaming servers')
  .option('-w, --ws-port <port>', 'WebSocket port', '3001')
  .option('-s, --sse-port <port>', 'SSE port', '3002')
  .action(async (options: any) => {
    console.log(chalk.bold('Starting MidStream servers...\n'));

    const wsPort = parseInt(options.wsPort);
    const ssePort = parseInt(options.ssePort);

    const wsServer = new WebSocketStreamServer(wsPort);
    const sseServer = new SSEStreamServer(ssePort);

    try {
      await wsServer.start();
      console.log(chalk.green(`✓ WebSocket server: ws://localhost:${wsPort}`));

      await sseServer.start();
      console.log(chalk.green(`✓ SSE server: http://localhost:${ssePort}`));

      console.log(chalk.bold('\nServers running. Press Ctrl+C to stop.\n'));

      // Handle graceful shutdown
      process.on('SIGINT', async () => {
        console.log(chalk.yellow('\nShutting down servers...'));
        await wsServer.stop();
        await sseServer.stop();
        process.exit(0);
      });

      // Keep process alive
      await new Promise(() => {});
    } catch (error) {
      console.error(chalk.red('Failed to start servers:'));
      console.error(chalk.red(error instanceof Error ? error.message : String(error)));
      process.exit(1);
    }
  });

// ============================================================================
// MCP command - Start MCP server
// ============================================================================

program
  .command('mcp')
  .description('Start MCP (Model Context Protocol) server')
  .action(async () => {
    console.log(chalk.bold('Starting MidStream MCP Server...\n'));

    const server = new MidStreamMCPServer();

    try {
      await server.start();

      // Handle graceful shutdown
      process.on('SIGINT', async () => {
        console.error(chalk.yellow('\nShutting down MCP server...'));
        await server.stop();
        process.exit(0);
      });
    } catch (error) {
      console.error(chalk.red('Failed to start MCP server:'));
      console.error(chalk.red(error instanceof Error ? error.message : String(error)));
      process.exit(1);
    }
  });

// ============================================================================
// Interactive mode
// ============================================================================

program
  .command('interactive')
  .alias('i')
  .description('Start interactive mode')
  .action(async () => {
    console.log(chalk.bold.cyan('MidStream Interactive Mode\n'));

    const agent = new MidStreamAgent();
    let running = true;

    while (running) {
      const { action } = await inquirer.prompt([
        {
          type: 'list',
          name: 'action',
          message: 'What would you like to do?',
          choices: [
            'Process message',
            'Analyze conversation',
            'Compare sequences',
            'Check status',
            'Exit',
          ],
        },
      ]);

      switch (action) {
        case 'Process message':
          const { message } = await inquirer.prompt([
            {
              type: 'input',
              name: 'message',
              message: 'Enter message:',
            },
          ]);
          const result = agent.processMessage(message);
          console.log(chalk.bold('\nResult:'));
          console.log(JSON.stringify(result, null, 2));
          console.log();
          break;

        case 'Analyze conversation':
          const { file } = await inquirer.prompt([
            {
              type: 'input',
              name: 'file',
              message: 'Enter conversation file path:',
            },
          ]);
          try {
            const data = JSON.parse(fs.readFileSync(file, 'utf-8'));
            const messages = Array.isArray(data) ? data : data.messages;
            const analysis = agent.analyzeConversation(messages);
            console.log(chalk.bold('\nAnalysis:'));
            console.log(JSON.stringify(analysis, null, 2));
            console.log();
          } catch (error) {
            console.error(chalk.red('Error:', error));
          }
          break;

        case 'Compare sequences':
          const { file1, file2, algorithm } = await inquirer.prompt([
            {
              type: 'input',
              name: 'file1',
              message: 'First sequence file:',
            },
            {
              type: 'input',
              name: 'file2',
              message: 'Second sequence file:',
            },
            {
              type: 'list',
              name: 'algorithm',
              message: 'Algorithm:',
              choices: ['dtw', 'lcs', 'edit', 'correlation'],
            },
          ]);
          try {
            const seq1 = JSON.parse(fs.readFileSync(file1, 'utf-8'));
            const seq2 = JSON.parse(fs.readFileSync(file2, 'utf-8'));
            const similarity = agent.compareSequences(seq1, seq2, algorithm);
            console.log(chalk.bold(`\nSimilarity: ${similarity.toFixed(4)}\n`));
          } catch (error) {
            console.error(chalk.red('Error:', error));
          }
          break;

        case 'Check status':
          const status = agent.getStatus();
          console.log(chalk.bold('\nAgent Status:'));
          console.log(JSON.stringify(status, null, 2));
          console.log();
          break;

        case 'Exit':
          running = false;
          console.log(chalk.cyan('\nGoodbye!\n'));
          break;
      }
    }
  });

// ============================================================================
// Benchmark command
// ============================================================================

program
  .command('benchmark')
  .description('Run performance benchmarks')
  .option('-s, --size <size>', 'Sequence size for benchmarks', '100')
  .option('-i, --iterations <iterations>', 'Number of iterations', '1000')
  .action(async (options: any) => {
    const size = parseInt(options.size);
    const iterations = parseInt(options.iterations);

    console.log(chalk.bold('Running benchmarks...\n'));

    try {
      const wasm = require('../wasm/midstream_wasm');

      const dtwTime = wasm.benchmark_dtw(size, iterations);
      const lcsTime = wasm.benchmark_lcs(size, iterations);

      console.log(chalk.bold('Benchmark Results:'));
      console.log(`Sequence size: ${size}`);
      console.log(`Iterations: ${iterations}\n`);

      console.log(`DTW: ${dtwTime.toFixed(3)}ms per iteration`);
      console.log(`LCS: ${lcsTime.toFixed(3)}ms per iteration`);

      if (dtwTime < 10) {
        console.log(chalk.green('\n✓ Excellent performance'));
      } else if (dtwTime < 50) {
        console.log(chalk.yellow('\n⚠ Good performance'));
      } else {
        console.log(chalk.red('\n✗ Consider optimization'));
      }
    } catch (error) {
      console.error(chalk.red('Benchmark failed:'));
      console.error(chalk.red(error instanceof Error ? error.message : String(error)));
      process.exit(1);
    }
  });

// Parse command line
program.parse();
