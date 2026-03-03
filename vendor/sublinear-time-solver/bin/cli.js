#!/usr/bin/env node

const { Command } = require('commander');
const chalk = require('chalk');
const ora = require('ora');
const express = require('express');
const fs = require('fs').promises;
const path = require('path');
const { createSolver } = require('../src/solver');
const { SolverServer } = require('../server/index');
const { FlowNexusIntegration } = require('../integrations/flow-nexus');

const program = new Command();

program
  .name('sublinear-time-solver')
  .description('Advanced Sublinear Time Sparse Linear System Solver')
  .version(require('../package.json').version || '1.0.0')
  .option('-v, --verbose', 'Enable verbose output')
  .option('-q, --quiet', 'Suppress non-essential output')
  .option('--debug', 'Enable debug mode');

// Solve command
program
  .command('solve')
  .description('Solve linear system Ax = b')
  .requiredOption('-m, --matrix <file>', 'Input matrix file (JSON, CSV, MTX)')
  .option('-b, --vector <file>', 'Right-hand side vector file')
  .option('-o, --output <file>', 'Output solution file')
  .option('--method <name>', 'Solver method (jacobi|gauss-seidel|cg|hybrid)', 'adaptive')
  .option('--tolerance <value>', 'Convergence tolerance', '1e-10')
  .option('--max-iterations <n>', 'Maximum iterations', '1000')
  .option('--streaming', 'Enable streaming output')
  .option('--verify', 'Enable solution verification')
  .action(async (options) => {
    const spinner = ora('Loading matrix...').start();

    try {
      // Load matrix data
      const matrixData = await loadMatrix(options.matrix);
      spinner.text = 'Loading vector...';

      // Load or generate vector
      const vector = options.vector
        ? await loadVector(options.vector)
        : generateRandomVector(matrixData.rows);

      spinner.succeed(`Loaded ${matrixData.rows}×${matrixData.cols} system`);

      // Create solver
      const solver = await createSolver({
        matrix: matrixData,
        method: options.method,
        tolerance: parseFloat(options.tolerance),
        maxIterations: parseInt(options.maxIterations),
        enableVerification: options.verify
      });

      if (options.streaming) {
        console.log(chalk.blue('🔄 Starting streaming solve...'));
        await streamingSolve(solver, vector, options);
      } else {
        console.log(chalk.blue('🔄 Starting batch solve...'));
        await batchSolve(solver, vector, options);
      }

    } catch (error) {
      spinner.fail('Solve failed');
      console.error(chalk.red('Error:'), error.message);
      if (options.debug) console.error(error.stack);
      process.exit(1);
    }
  });

// Serve command
program
  .command('serve')
  .description('Start HTTP streaming server')
  .option('-p, --port <number>', 'Server port', '3000')
  .option('--cors', 'Enable CORS')
  .option('--flow-nexus', 'Enable Flow-Nexus integration')
  .option('--workers <number>', 'Number of worker threads', '1')
  .option('--max-sessions <number>', 'Maximum concurrent sessions', '100')
  .option('--auth-token <token>', 'Authentication token for protected endpoints')
  .action(async (options) => {
    const server = new SolverServer({
      port: parseInt(options.port),
      cors: options.cors,
      workers: parseInt(options.workers),
      maxSessions: parseInt(options.maxSessions),
      authToken: options.authToken,
      flowNexusEnabled: options.flowNexus
    });

    await server.start();
    console.log(chalk.green(`🚀 Solver server running on port ${options.port}`));
    console.log(chalk.blue(`   REST API: http://localhost:${options.port}/api`));
    console.log(chalk.blue(`   WebSocket: ws://localhost:${options.port}/ws`));

    if (options.flowNexus) {
      console.log(chalk.yellow(`   Flow-Nexus: Enabled`));
    }
  });

// Verify command
program
  .command('verify')
  .description('Verify solution accuracy')
  .requiredOption('-m, --matrix <file>', 'Matrix file')
  .requiredOption('-x, --solution <file>', 'Solution vector file')
  .requiredOption('-b, --vector <file>', 'Right-hand side vector file')
  .option('--tolerance <value>', 'Verification tolerance', '1e-8')
  .option('--probes <count>', 'Number of random probes', '10')
  .action(async (options) => {
    const spinner = ora('Loading verification data...').start();

    try {
      const matrix = await loadMatrix(options.matrix);
      const solution = await loadVector(options.solution);
      const vector = await loadVector(options.vector);

      spinner.text = 'Running verification...';

      const result = await verifySolution({
        matrix,
        solution,
        vector,
        tolerance: parseFloat(options.tolerance),
        probes: parseInt(options.probes)
      });

      if (result.verified) {
        spinner.succeed('Solution verified successfully');
        console.log(chalk.green(`✓ Max error: ${result.maxError.toExponential(2)}`));
        console.log(chalk.green(`✓ Mean error: ${result.meanError.toExponential(2)}`));
      } else {
        spinner.fail('Solution verification failed');
        console.log(chalk.red(`✗ Max error: ${result.maxError.toExponential(2)}`));
        console.log(chalk.red(`✗ Tolerance: ${options.tolerance}`));
      }

    } catch (error) {
      spinner.fail('Verification failed');
      console.error(chalk.red('Error:'), error.message);
      process.exit(1);
    }
  });

// Benchmark command
program
  .command('benchmark')
  .description('Run performance benchmarks')
  .option('--size <number>', 'Matrix size', '1000')
  .option('--sparsity <value>', 'Matrix sparsity (0-1)', '0.01')
  .option('--methods <list>', 'Comma-separated list of methods', 'jacobi,cg,hybrid')
  .option('--iterations <number>', 'Benchmark iterations', '5')
  .option('--output <file>', 'Output results to JSON file')
  .action(async (options) => {
    const spinner = ora('Setting up benchmark...').start();

    try {
      const results = await runBenchmark({
        size: parseInt(options.size),
        sparsity: parseFloat(options.sparsity),
        methods: options.methods.split(','),
        iterations: parseInt(options.iterations)
      });

      spinner.succeed('Benchmark completed');

      console.log(chalk.blue('\n📊 Benchmark Results:'));
      results.forEach(result => {
        console.log(chalk.white(`\n${result.method}:`));
        console.log(`  Average time: ${result.avgTime.toFixed(2)}ms`);
        console.log(`  Min time: ${result.minTime.toFixed(2)}ms`);
        console.log(`  Max time: ${result.maxTime.toFixed(2)}ms`);
        console.log(`  Iterations: ${result.avgIterations.toFixed(0)}`);
        console.log(`  Convergence rate: ${(result.convergenceRate * 100).toFixed(1)}%`);
      });

      if (options.output) {
        await fs.writeFile(options.output, JSON.stringify(results, null, 2));
        console.log(chalk.green(`\n📁 Results saved to ${options.output}`));
      }

    } catch (error) {
      spinner.fail('Benchmark failed');
      console.error(chalk.red('Error:'), error.message);
      process.exit(1);
    }
  });

// Convert command
program
  .command('convert')
  .description('Convert between matrix formats')
  .requiredOption('-i, --input <file>', 'Input file')
  .requiredOption('-o, --output <file>', 'Output file')
  .option('--format <type>', 'Output format (json|csv|mtx|binary)', 'json')
  .option('--compress', 'Compress output')
  .action(async (options) => {
    const spinner = ora('Converting matrix format...').start();

    try {
      await convertMatrix({
        input: options.input,
        output: options.output,
        format: options.format,
        compress: options.compress
      });

      spinner.succeed(`Converted ${options.input} → ${options.output}`);

    } catch (error) {
      spinner.fail('Conversion failed');
      console.error(chalk.red('Error:'), error.message);
      process.exit(1);
    }
  });

// Flow-Nexus integration command
program
  .command('flow-nexus')
  .description('Flow-Nexus platform integration')
  .option('--register', 'Register as solver service')
  .option('--swarm-join <id>', 'Join swarm with specified ID')
  .option('--endpoint <url>', 'Flow-Nexus endpoint URL')
  .option('--token <token>', 'Authentication token')
  .action(async (options) => {
    const spinner = ora('Connecting to Flow-Nexus...').start();

    try {
      const integration = new FlowNexusIntegration({
        endpoint: options.endpoint,
        token: options.token
      });

      if (options.register) {
        await integration.registerSolver();
        spinner.succeed('Registered with Flow-Nexus platform');
      }

      if (options.swarmJoin) {
        await integration.joinSwarm(options.swarmJoin);
        spinner.succeed(`Joined swarm: ${options.swarmJoin}`);
      }

    } catch (error) {
      spinner.fail('Flow-Nexus integration failed');
      console.error(chalk.red('Error:'), error.message);
      process.exit(1);
    }
  });

// Helper functions
async function loadMatrix(filepath) {
  const data = await fs.readFile(filepath, 'utf8');
  const ext = path.extname(filepath).toLowerCase();

  switch (ext) {
    case '.json':
      return JSON.parse(data);
    case '.csv':
      return parseCSVMatrix(data);
    case '.mtx':
      return parseMatrixMarket(data);
    default:
      throw new Error(`Unsupported matrix format: ${ext}`);
  }
}

async function loadVector(filepath) {
  const data = await fs.readFile(filepath, 'utf8');
  const ext = path.extname(filepath).toLowerCase();

  switch (ext) {
    case '.json':
      return JSON.parse(data);
    case '.csv':
      return data.split('\n').map(line => parseFloat(line.trim())).filter(x => !isNaN(x));
    default:
      throw new Error(`Unsupported vector format: ${ext}`);
  }
}

function generateRandomVector(size) {
  return Array.from({ length: size }, () => Math.random() * 10 - 5);
}

async function streamingSolve(solver, vector, options) {
  const outputStream = options.output ? require('fs').createWriteStream(options.output) : null;

  for await (const update of solver.streamSolve(vector)) {
    const line = JSON.stringify({
      iteration: update.iteration,
      residual: update.residual,
      timestamp: new Date().toISOString(),
      convergence_rate: update.convergenceRate,
      memory_usage: update.memoryUsage
    }) + '\n';

    if (outputStream) {
      outputStream.write(line);
    } else {
      console.log(chalk.gray(`[${update.iteration}]`),
                  chalk.blue(`Residual: ${update.residual.toExponential(2)}`));
    }

    if (update.converged) {
      console.log(chalk.green(`✓ Converged in ${update.iteration} iterations`));
      break;
    }
  }

  if (outputStream) {
    outputStream.end();
    console.log(chalk.green(`📁 Streaming output saved to ${options.output}`));
  }
}

async function batchSolve(solver, vector, options) {
  const startTime = Date.now();

  const solution = await solver.solve(vector, {
    onProgress: (update) => {
      if (!options.quiet) {
        process.stdout.write(`\r${chalk.blue('Progress:')} Iteration ${update.iteration}, Residual: ${update.residual.toExponential(2)}`);
      }
    }
  });

  const elapsed = Date.now() - startTime;

  console.log(chalk.green(`\n✓ Solution found in ${elapsed}ms`));
  console.log(`  Iterations: ${solution.iterations}`);
  console.log(`  Final residual: ${solution.residual.toExponential(2)}`);
  console.log(`  Memory usage: ${solution.memoryUsage}MB`);

  if (options.output) {
    await fs.writeFile(options.output, JSON.stringify({
      solution: solution.values,
      metadata: {
        iterations: solution.iterations,
        residual: solution.residual,
        solveTime: elapsed,
        method: options.method
      }
    }, null, 2));

    console.log(chalk.green(`📁 Solution saved to ${options.output}`));
  }
}

async function verifySolution({ matrix, solution, vector, tolerance, probes }) {
  // Implement verification logic
  const errors = [];

  // Full residual check: ||Ax - b||
  const residual = computeResidual(matrix, solution, vector);
  const residualNorm = vectorNorm(residual);

  // Random probe verification
  for (let i = 0; i < probes; i++) {
    const idx = Math.floor(Math.random() * matrix.rows);
    const computed = multiplyMatrixRow(matrix, idx, solution);
    const error = Math.abs(computed - vector[idx]);
    errors.push(error);
  }

  const maxError = Math.max(...errors);
  const meanError = errors.reduce((a, b) => a + b) / errors.length;

  return {
    verified: maxError < tolerance && residualNorm < tolerance,
    maxError,
    meanError,
    residualNorm,
    probeErrors: errors
  };
}

async function runBenchmark({ size, sparsity, methods, iterations }) {
  const results = [];

  for (const method of methods) {
    const times = [];
    const iterationCounts = [];
    let convergenceCount = 0;

    for (let i = 0; i < iterations; i++) {
      const matrix = generateRandomSparseMatrix(size, sparsity);
      const vector = generateRandomVector(size);

      const solver = await createSolver({ matrix, method });
      const startTime = Date.now();

      try {
        const solution = await solver.solve(vector);
        const elapsed = Date.now() - startTime;

        times.push(elapsed);
        iterationCounts.push(solution.iterations);
        if (solution.converged) convergenceCount++;
      } catch (error) {
        console.warn(`Benchmark failed for ${method}:`, error.message);
      }
    }

    if (times.length > 0) {
      results.push({
        method,
        avgTime: times.reduce((a, b) => a + b) / times.length,
        minTime: Math.min(...times),
        maxTime: Math.max(...times),
        avgIterations: iterationCounts.reduce((a, b) => a + b) / iterationCounts.length,
        convergenceRate: convergenceCount / iterations
      });
    }
  }

  return results;
}

async function convertMatrix({ input, output, format, compress }) {
  const matrix = await loadMatrix(input);
  let converted;

  switch (format.toLowerCase()) {
    case 'json':
      converted = JSON.stringify(matrix, null, compress ? 0 : 2);
      break;
    case 'csv':
      converted = matrixToCSV(matrix);
      break;
    case 'mtx':
      converted = matrixToMatrixMarket(matrix);
      break;
    case 'binary':
      converted = matrixToBinary(matrix);
      break;
    default:
      throw new Error(`Unsupported output format: ${format}`);
  }

  await fs.writeFile(output, converted);
}

// Matrix parsing utilities
function parseCSVMatrix(data) {
  const lines = data.trim().split('\n');
  const matrix = lines.map(line =>
    line.split(',').map(val => parseFloat(val.trim()))
  );

  return {
    rows: matrix.length,
    cols: matrix[0].length,
    data: matrix,
    format: 'dense'
  };
}

function parseMatrixMarket(data) {
  const lines = data.trim().split('\n');
  let headerLine = 0;

  // Skip comments
  while (lines[headerLine].startsWith('%')) headerLine++;

  const [rows, cols, entries] = lines[headerLine].split(' ').map(Number);
  const values = [];
  const rowIndices = [];
  const colIndices = [];

  for (let i = headerLine + 1; i < lines.length; i++) {
    if (lines[i].trim()) {
      const [row, col, val] = lines[i].split(' ');
      rowIndices.push(parseInt(row) - 1); // Convert to 0-based
      colIndices.push(parseInt(col) - 1);
      values.push(parseFloat(val));
    }
  }

  return {
    rows,
    cols,
    entries,
    data: { values, rowIndices, colIndices },
    format: 'coo'
  };
}

// Mathematical utilities
function computeResidual(matrix, x, b) {
  const Ax = multiplyMatrixVector(matrix, x);
  return Ax.map((val, i) => val - b[i]);
}

function vectorNorm(v) {
  return Math.sqrt(v.reduce((sum, val) => sum + val * val, 0));
}

function multiplyMatrixVector(matrix, vector) {
  if (matrix.format === 'dense') {
    return matrix.data.map(row =>
      row.reduce((sum, val, i) => sum + val * vector[i], 0)
    );
  } else if (matrix.format === 'coo') {
    const result = new Array(matrix.rows).fill(0);
    for (let i = 0; i < matrix.data.values.length; i++) {
      const row = matrix.data.rowIndices[i];
      const col = matrix.data.colIndices[i];
      const val = matrix.data.values[i];
      result[row] += val * vector[col];
    }
    return result;
  }
  throw new Error(`Unsupported matrix format: ${matrix.format}`);
}

function multiplyMatrixRow(matrix, rowIndex, vector) {
  if (matrix.format === 'dense') {
    return matrix.data[rowIndex].reduce((sum, val, i) => sum + val * vector[i], 0);
  } else if (matrix.format === 'coo') {
    let result = 0;
    for (let i = 0; i < matrix.data.values.length; i++) {
      if (matrix.data.rowIndices[i] === rowIndex) {
        const col = matrix.data.colIndices[i];
        const val = matrix.data.values[i];
        result += val * vector[col];
      }
    }
    return result;
  }
  throw new Error(`Unsupported matrix format: ${matrix.format}`);
}

function generateRandomSparseMatrix(size, sparsity) {
  const values = [];
  const rowIndices = [];
  const colIndices = [];

  const numEntries = Math.floor(size * size * sparsity);

  for (let i = 0; i < numEntries; i++) {
    const row = Math.floor(Math.random() * size);
    const col = Math.floor(Math.random() * size);
    const val = Math.random() * 10 - 5;

    rowIndices.push(row);
    colIndices.push(col);
    values.push(val);
  }

  return {
    rows: size,
    cols: size,
    entries: numEntries,
    data: { values, rowIndices, colIndices },
    format: 'coo'
  };
}

// Format conversion utilities
function matrixToCSV(matrix) {
  if (matrix.format === 'dense') {
    return matrix.data.map(row => row.join(',')).join('\n');
  }
  throw new Error('CSV export only supported for dense matrices');
}

function matrixToMatrixMarket(matrix) {
  let output = '%%MatrixMarket matrix coordinate real general\n';
  output += `${matrix.rows} ${matrix.cols} ${matrix.entries || matrix.data.values.length}\n`;

  if (matrix.format === 'coo') {
    for (let i = 0; i < matrix.data.values.length; i++) {
      const row = matrix.data.rowIndices[i] + 1; // Convert to 1-based
      const col = matrix.data.colIndices[i] + 1;
      const val = matrix.data.values[i];
      output += `${row} ${col} ${val}\n`;
    }
  }

  return output;
}

function matrixToBinary(matrix) {
  // Simplified binary format
  const buffer = Buffer.alloc(8 + matrix.data.values.length * 8);
  buffer.writeInt32LE(matrix.rows, 0);
  buffer.writeInt32LE(matrix.cols, 4);

  for (let i = 0; i < matrix.data.values.length; i++) {
    buffer.writeDoubleLE(matrix.data.values[i], 8 + i * 8);
  }

  return buffer;
}

// Error handling
process.on('uncaughtException', (error) => {
  console.error(chalk.red('Uncaught Exception:'), error.message);
  if (program.opts().debug) console.error(error.stack);
  process.exit(1);
});

process.on('unhandledRejection', (error) => {
  console.error(chalk.red('Unhandled Rejection:'), error.message);
  if (program.opts().debug) console.error(error.stack);
  process.exit(1);
});

// Parse command line arguments
program.parseAsync().catch((error) => {
  console.error(chalk.red('CLI Error:'), error.message);
  process.exit(1);
});