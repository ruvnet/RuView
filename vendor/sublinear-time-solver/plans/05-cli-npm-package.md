# CLI Interface & NPM Package Plan

## Overview

This plan outlines the creation of a comprehensive CLI interface and npm package for the sublinear time solver library, providing both command-line tools and programmatic APIs with excellent developer experience.

## 1. CLI Command Structure

### Core Commands

```bash
# Basic solving
npx add-solver solve --matrix graph.json --vector b.csv --output solution.json
npx add-solver solve --input system.mtx --method hybrid --precision 1e-10

# Server mode
npx add-solver serve --port 8080 --method adaptive --cors
npx add-solver serve --config server.yaml --workers 4

# Verification and analysis
npx add-solver verify --solution x.json --system A.json --tolerance 1e-8
npx add-solver analyze --matrix sparse.mtx --report stats.json

# Benchmarking
npx add-solver benchmark --size 10000 --sparsity 0.01 --methods all
npx add-solver benchmark --suite performance --output bench.json

# Utilities
npx add-solver convert --input matrix.csv --output sparse.mtx --format mm
npx add-solver validate --matrix A.json --schema
npx add-solver optimize --matrix large.mtx --method auto --profile
```

### Command Structure Details

```bash
add-solver <command> [options]

Commands:
  solve      Solve a linear system
  serve      Start HTTP server for streaming solutions
  verify     Verify solution accuracy
  analyze    Analyze matrix properties
  benchmark  Run performance benchmarks
  convert    Convert between matrix formats
  validate   Validate input files
  optimize   Find optimal solver configuration
  init       Initialize project with templates
  config     Manage configuration settings

Global Options:
  --config, -c     Configuration file path
  --verbose, -v    Verbose output
  --quiet, -q      Suppress non-essential output
  --help, -h       Show help
  --version        Show version
  --debug          Enable debug mode
```

## 2. Input/Output Formats

### Supported Matrix Formats

```typescript
// Matrix Market Format (.mtx)
interface MatrixMarketFormat {
  header: string;
  rows: number;
  cols: number;
  entries: number;
  data: Array<[number, number, number]>; // [row, col, value]
}

// JSON Matrix Format
interface JSONMatrixFormat {
  type: 'sparse' | 'dense';
  dimensions: [number, number];
  data: {
    indices?: number[][];
    values: number[];
    format?: 'coo' | 'csr' | 'csc';
  };
  metadata?: {
    name?: string;
    description?: string;
    properties?: string[];
  };
}

// CSV Vector Format
interface CSVVectorFormat {
  headers?: boolean;
  delimiter: ',' | ';' | '\t';
  values: number[];
}

// Binary Format (HDF5-like)
interface BinaryFormat {
  magic: 'ADDSLV01';
  compression: 'none' | 'gzip' | 'lz4';
  precision: 'float32' | 'float64';
  chunks: BufferChunk[];
}
```

### Format Conversion Examples

```bash
# Matrix Market to JSON
npx add-solver convert \
  --input matrix.mtx \
  --output matrix.json \
  --format json \
  --compress

# CSV to Binary
npx add-solver convert \
  --input vectors.csv \
  --output vectors.bin \
  --format binary \
  --precision float64

# Auto-detect format
npx add-solver convert \
  --input data.* \
  --output optimized/ \
  --auto-format \
  --batch
```

## 3. CLI Implementation

### Main CLI Entry Point (src/cli/index.ts)

```typescript
#!/usr/bin/env node

import { Command } from 'commander';
import chalk from 'chalk';
import ora from 'ora';
import { createSolveCommand } from './commands/solve';
import { createServeCommand } from './commands/serve';
import { createBenchmarkCommand } from './commands/benchmark';
import { loadConfig } from './config';
import { setupLogging } from './utils/logging';

const program = new Command();

program
  .name('add-solver')
  .description('Sublinear Time Sparse Linear System Solver')
  .version(process.env.npm_package_version || '1.0.0')
  .option('-c, --config <path>', 'Configuration file path')
  .option('-v, --verbose', 'Verbose output')
  .option('-q, --quiet', 'Quiet mode')
  .option('--debug', 'Debug mode')
  .hook('preAction', async (thisCommand) => {
    const opts = thisCommand.opts();
    setupLogging(opts);

    if (opts.config) {
      await loadConfig(opts.config);
    }
  });

// Add commands
createSolveCommand(program);
createServeCommand(program);
createBenchmarkCommand(program);

program.parseAsync().catch((error) => {
  console.error(chalk.red('Error:'), error.message);
  process.exit(1);
});
```

### Solve Command (src/cli/commands/solve.ts)

```typescript
import { Command } from 'commander';
import { createReadStream, createWriteStream } from 'fs';
import { pipeline } from 'stream/promises';
import { Solver, MatrixLoader, VectorLoader } from '../../core';
import { ProgressBar } from '../utils/progress';
import { formatDuration, formatBytes } from '../utils/format';

export function createSolveCommand(program: Command) {
  program
    .command('solve')
    .description('Solve linear system Ax = b')
    .requiredOption('-m, --matrix <path>', 'Input matrix file')
    .option('-b, --vector <path>', 'Right-hand side vector')
    .option('-o, --output <path>', 'Output solution file')
    .option('--method <name>', 'Solver method', 'adaptive')
    .option('--precision <value>', 'Convergence tolerance', '1e-10')
    .option('--max-iterations <n>', 'Maximum iterations', '1000')
    .option('--streaming', 'Enable streaming output')
    .option('--progress', 'Show progress bar', true)
    .action(async (options) => {
      const spinner = ora('Loading matrix...').start();

      try {
        // Load matrix
        const matrix = await MatrixLoader.load(options.matrix);
        spinner.text = 'Loading vector...';

        // Load or generate vector
        const vector = options.vector
          ? await VectorLoader.load(options.vector)
          : matrix.generateRandomVector();

        spinner.succeed(`Loaded ${matrix.dimensions[0]}x${matrix.dimensions[1]} system`);

        // Create solver
        const solver = await Solver.create({
          matrix,
          method: options.method,
          tolerance: parseFloat(options.precision),
          maxIterations: parseInt(options.maxIterations)
        });

        // Setup progress tracking
        const progress = new ProgressBar({
          total: options.maxIterations,
          format: 'Solving [{bar}] {percentage}% | Residual: {residual} | ETA: {eta}s'
        });

        if (options.streaming && options.output) {
          // Streaming mode
          const outputStream = createWriteStream(options.output);
          await pipeline(
            solver.streamSolve(vector),
            async function* (source) {
              for await (const update of source) {
                progress.update(update.iteration, { residual: update.residual.toExponential(2) });
                yield JSON.stringify(update) + '\n';
              }
            },
            outputStream
          );
        } else {
          // Regular mode
          const solution = await solver.solve(vector, {
            onProgress: (update) => {
              progress.update(update.iteration, { residual: update.residual.toExponential(2) });
            }
          });

          progress.stop();

          if (options.output) {
            await solution.save(options.output);
            console.log(`Solution saved to ${options.output}`);
          } else {
            console.log('Solution:', solution.values.slice(0, 10), '...');
          }
        }

      } catch (error) {
        spinner.fail('Solving failed');
        throw error;
      }
    });
}
```

### Server Command (src/cli/commands/serve.ts)

```typescript
import { Command } from 'commander';
import express from 'express';
import cors from 'cors';
import { WebSocketServer } from 'ws';
import { SolverService } from '../../server/solver-service';
import { setupMiddleware } from '../../server/middleware';

export function createServeCommand(program: Command) {
  program
    .command('serve')
    .description('Start HTTP server for streaming solutions')
    .option('-p, --port <number>', 'Server port', '8080')
    .option('--cors', 'Enable CORS', false)
    .option('--workers <number>', 'Number of worker threads', '1')
    .option('--max-matrix-size <bytes>', 'Maximum matrix size', '100MB')
    .option('--rate-limit <requests>', 'Requests per minute', '100')
    .action(async (options) => {
      const app = express();
      const port = parseInt(options.port);

      // Setup middleware
      setupMiddleware(app, {
        cors: options.cors,
        rateLimit: parseInt(options.rateLimit),
        maxMatrixSize: options.maxMatrixSize
      });

      // Create solver service
      const solverService = new SolverService({
        workers: parseInt(options.workers)
      });

      // REST API endpoints
      app.post('/api/solve', async (req, res) => {
        try {
          const { matrix, vector, options: solveOptions } = req.body;
          const jobId = await solverService.submitJob(matrix, vector, solveOptions);
          res.json({ jobId, status: 'submitted' });
        } catch (error) {
          res.status(400).json({ error: error.message });
        }
      });

      app.get('/api/solve/:jobId', async (req, res) => {
        const status = await solverService.getJobStatus(req.params.jobId);
        res.json(status);
      });

      app.get('/api/solve/:jobId/stream', async (req, res) => {
        res.writeHead(200, {
          'Content-Type': 'text/event-stream',
          'Cache-Control': 'no-cache',
          'Connection': 'keep-alive'
        });

        const stream = solverService.getJobStream(req.params.jobId);
        for await (const update of stream) {
          res.write(`data: ${JSON.stringify(update)}\n\n`);
        }
        res.end();
      });

      // WebSocket support
      const server = app.listen(port, () => {
        console.log(`🚀 Solver server running on port ${port}`);
        console.log(`   REST API: http://localhost:${port}/api`);
        console.log(`   WebSocket: ws://localhost:${port}/ws`);
      });

      const wss = new WebSocketServer({ server, path: '/ws' });
      setupWebSocketHandlers(wss, solverService);
    });
}
```

## 4. NPM Package Structure

### Package.json Configuration

```json
{
  "name": "@myorg/add-solver",
  "version": "1.0.0",
  "description": "Sublinear Time Sparse Linear System Solver",
  "keywords": ["linear-algebra", "sparse-matrix", "solver", "numerical", "optimization"],
  "author": "Your Organization",
  "license": "MIT",
  "homepage": "https://github.com/myorg/add-solver",
  "repository": {
    "type": "git",
    "url": "https://github.com/myorg/add-solver.git"
  },
  "bugs": {
    "url": "https://github.com/myorg/add-solver/issues"
  },
  "main": "dist/index.js",
  "module": "dist/index.mjs",
  "types": "dist/index.d.ts",
  "bin": {
    "add-solver": "dist/cli/index.js"
  },
  "exports": {
    ".": {
      "import": "./dist/index.mjs",
      "require": "./dist/index.js",
      "types": "./dist/index.d.ts"
    },
    "./server": {
      "import": "./dist/server/index.mjs",
      "require": "./dist/server/index.js",
      "types": "./dist/server/index.d.ts"
    },
    "./cli": {
      "import": "./dist/cli/index.mjs",
      "require": "./dist/cli/index.js"
    }
  },
  "files": [
    "dist/",
    "README.md",
    "LICENSE",
    "CHANGELOG.md"
  ],
  "engines": {
    "node": ">=16.0.0"
  },
  "scripts": {
    "build": "rollup -c && chmod +x dist/cli/index.js",
    "build:types": "tsc --emitDeclarationOnly",
    "test": "jest",
    "test:ci": "jest --coverage --ci",
    "lint": "eslint src/**/*.ts",
    "format": "prettier --write src/**/*.ts",
    "docs": "typedoc src/index.ts",
    "prepublishOnly": "npm run build && npm run test",
    "cli": "node dist/cli/index.js"
  },
  "dependencies": {
    "commander": "^9.4.1",
    "chalk": "^5.0.0",
    "ora": "^6.1.2",
    "express": "^4.18.2",
    "ws": "^8.11.0",
    "cors": "^2.8.5",
    "compression": "^1.7.4",
    "helmet": "^6.0.1"
  },
  "devDependencies": {
    "@types/node": "^18.0.0",
    "@types/express": "^4.17.15",
    "@types/ws": "^8.5.3",
    "@types/jest": "^29.2.5",
    "typescript": "^4.9.4",
    "rollup": "^3.7.5",
    "jest": "^29.3.1",
    "eslint": "^8.30.0",
    "prettier": "^2.8.1",
    "typedoc": "^0.23.21"
  },
  "peerDependencies": {
    "numpy": "^1.0.0"
  },
  "optionalDependencies": {
    "@myorg/add-solver-gpu": "^1.0.0",
    "@myorg/add-solver-wasm": "^1.0.0"
  }
}
```

### TypeScript Configuration (tsconfig.json)

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "node",
    "lib": ["ES2020", "DOM"],
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "removeComments": false,
    "experimentalDecorators": true,
    "emitDecoratorMetadata": true,
    "resolveJsonModule": true,
    "allowSyntheticDefaultImports": true
  },
  "include": [
    "src/**/*"
  ],
  "exclude": [
    "node_modules",
    "dist",
    "**/*.test.ts",
    "**/*.spec.ts"
  ]
}
```

### Build Configuration (rollup.config.js)

```javascript
import typescript from '@rollup/plugin-typescript';
import resolve from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';
import json from '@rollup/plugin-json';
import { terser } from 'rollup-plugin-terser';

const external = ['express', 'ws', 'commander', 'chalk', 'ora'];

export default [
  // Main library bundle
  {
    input: 'src/index.ts',
    output: [
      {
        file: 'dist/index.js',
        format: 'cjs',
        sourcemap: true
      },
      {
        file: 'dist/index.mjs',
        format: 'es',
        sourcemap: true
      }
    ],
    external,
    plugins: [
      resolve(),
      commonjs(),
      json(),
      typescript({ tsconfig: './tsconfig.json' })
    ]
  },
  // CLI bundle
  {
    input: 'src/cli/index.ts',
    output: {
      file: 'dist/cli/index.js',
      format: 'cjs',
      sourcemap: true,
      banner: '#!/usr/bin/env node'
    },
    external,
    plugins: [
      resolve(),
      commonjs(),
      json(),
      typescript({ tsconfig: './tsconfig.json' }),
      terser()
    ]
  },
  // Server bundle
  {
    input: 'src/server/index.ts',
    output: [
      {
        file: 'dist/server/index.js',
        format: 'cjs',
        sourcemap: true
      },
      {
        file: 'dist/server/index.mjs',
        format: 'es',
        sourcemap: true
      }
    ],
    external,
    plugins: [
      resolve(),
      commonjs(),
      json(),
      typescript({ tsconfig: './tsconfig.json' })
    ]
  }
];
```

## 5. Developer Experience Features

### API Design (src/index.ts)

```typescript
// Main library exports
export { Solver, SolverOptions, SolverResult } from './core/solver';
export { Matrix, SparseMatrix, DenseMatrix } from './core/matrix';
export { Vector, SparseVector } from './core/vector';
export { MatrixLoader, VectorLoader } from './io/loaders';
export { SolverMethod, AdaptiveMethod, HybridMethod } from './methods';

// Type definitions for excellent IntelliSense
export interface SolverConfig {
  /** Solver method to use */
  method?: 'conjugate-gradient' | 'gmres' | 'bicgstab' | 'adaptive' | 'hybrid';
  /** Convergence tolerance */
  tolerance?: number;
  /** Maximum iterations */
  maxIterations?: number;
  /** Enable GPU acceleration if available */
  useGPU?: boolean;
  /** Preconditioner type */
  preconditioner?: 'jacobi' | 'ilu' | 'multigrid' | 'none';
  /** Memory limit in bytes */
  memoryLimit?: number;
}

export interface SolverProgress {
  iteration: number;
  residual: number;
  estimatedTimeRemaining: number;
  memoryUsage: number;
}

// Fluent API design
export class Solver {
  static async create(config: SolverConfig): Promise<Solver> {
    // Implementation
  }

  /** Configure solver options with method chaining */
  withTolerance(tolerance: number): Solver;
  withMaxIterations(iterations: number): Solver;
  withGPU(enable: boolean): Solver;
  withPreconditioner(type: string): Solver;

  /** Solve with progress callbacks */
  async solve(
    vector: Vector,
    options?: {
      onProgress?: (progress: SolverProgress) => void;
      signal?: AbortSignal;
    }
  ): Promise<SolverResult>;

  /** Stream solving with async iteration */
  async *streamSolve(vector: Vector): AsyncGenerator<SolverProgress, SolverResult>;

  /** Batch solve multiple systems */
  async solveBatch(vectors: Vector[]): Promise<SolverResult[]>;
}

// Error types with helpful messages
export class SolverError extends Error {
  constructor(
    message: string,
    public code: string,
    public suggestions: string[] = []
  ) {
    super(message);
  }
}

export class ConvergenceError extends SolverError {
  constructor(iterations: number, residual: number) {
    super(
      `Failed to converge after ${iterations} iterations (residual: ${residual})`,
      'CONVERGENCE_FAILED',
      [
        'Try increasing maxIterations',
        'Use a different preconditioner',
        'Check matrix conditioning with analyze command'
      ]
    );
  }
}
```

### Usage Examples and Documentation

```typescript
// examples/basic-usage.ts
import { Solver, MatrixLoader } from '@myorg/add-solver';

async function basicExample() {
  // Load matrix from file
  const matrix = await MatrixLoader.load('system.mtx');

  // Create solver with automatic method selection
  const solver = await Solver.create({
    method: 'adaptive',
    tolerance: 1e-10
  });

  // Generate random right-hand side
  const b = matrix.generateRandomVector();

  // Solve with progress tracking
  const solution = await solver.solve(b, {
    onProgress: (progress) => {
      console.log(`Iteration ${progress.iteration}: ${progress.residual}`);
    }
  });

  console.log('Solution found:', solution.values.slice(0, 5));
  console.log('Solve time:', solution.stats.solveTime);
  console.log('Iterations:', solution.stats.iterations);
}

// examples/streaming-example.ts
async function streamingExample() {
  const solver = await Solver.create({ method: 'conjugate-gradient' });
  const matrix = await MatrixLoader.load('large-system.mtx');
  const b = matrix.generateRandomVector();

  // Stream solving progress
  for await (const update of solver.streamSolve(b)) {
    // Update UI or log progress
    console.log(`Progress: ${update.iteration}/${solver.maxIterations}`);

    if (update.residual < 1e-6) {
      console.log('Early convergence achieved');
      break;
    }
  }
}

// examples/batch-solving.ts
async function batchExample() {
  const solver = await Solver.create({ method: 'hybrid' });
  const matrix = await MatrixLoader.load('system.mtx');

  // Generate multiple right-hand sides
  const vectors = Array.from({ length: 10 }, () => matrix.generateRandomVector());

  // Solve all systems efficiently
  const solutions = await solver.solveBatch(vectors);

  solutions.forEach((solution, i) => {
    console.log(`System ${i}: ${solution.stats.iterations} iterations`);
  });
}
```

### Debug and Development Helpers

```typescript
// src/utils/debug.ts
export class SolverDebugger {
  static analyzeMatrix(matrix: Matrix): MatrixAnalysis {
    return {
      conditioning: matrix.conditionNumber(),
      sparsity: matrix.sparsityPattern(),
      eigenvalues: matrix.estimateEigenvalues(),
      recommendations: matrix.getSolverRecommendations()
    };
  }

  static profileSolver(solver: Solver): SolverProfile {
    return {
      memoryUsage: solver.getMemoryUsage(),
      computeUnits: solver.getComputeUnits(),
      bottlenecks: solver.identifyBottlenecks(),
      optimizations: solver.suggestOptimizations()
    };
  }

  static visualizeConvergence(history: SolverProgress[]): string {
    // ASCII art convergence plot
    return generateConvergencePlot(history);
  }
}

// Error messages with solutions
export function createHelpfulError(error: Error, context: any): Error {
  const suggestions = [];

  if (error.message.includes('singular matrix')) {
    suggestions.push('Matrix appears to be singular. Try:');
    suggestions.push('  - Check for zero rows/columns');
    suggestions.push('  - Add regularization: matrix + λI');
    suggestions.push('  - Use least-squares solver instead');
  }

  if (error.message.includes('memory')) {
    suggestions.push('Memory limit exceeded. Try:');
    suggestions.push('  - Increase memoryLimit option');
    suggestions.push('  - Use iterative methods instead of direct');
    suggestions.push('  - Process matrix in chunks');
  }

  return new SolverError(error.message, 'SOLVER_ERROR', suggestions);
}
```

## 6. Testing & CI/CD

### GitHub Actions Workflow (.github/workflows/ci.yml)

```yaml
name: CI/CD Pipeline

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  NODE_VERSION: '18'

jobs:
  test:
    name: Test on ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
        node-version: ['16', '18', '20']

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Run linting
        run: npm run lint

      - name: Run type checking
        run: npm run type-check

      - name: Run unit tests
        run: npm run test:unit

      - name: Run integration tests
        run: npm run test:integration

      - name: Test CLI
        run: |
          npm run build
          ./dist/cli/index.js --version
          ./dist/cli/index.js solve --help

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        if: matrix.os == 'ubuntu-latest' && matrix.node-version == '18'

  benchmark:
    name: Performance Regression Tests
    runs-on: ubuntu-latest
    needs: test

    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Need history for comparison

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '18'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Build package
        run: npm run build

      - name: Run benchmarks
        run: npm run benchmark

      - name: Store benchmark results
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'customSmallerIsBetter'
          output-file-path: benchmark-results.json
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
          alert-threshold: '200%'
          comment-on-alert: true

  package-size:
    name: Package Size Check
    runs-on: ubuntu-latest
    needs: test

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '18'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci

      - name: Build package
        run: npm run build

      - name: Check package size
        run: |
          npm pack
          PACKAGE_SIZE=$(stat -c%s *.tgz)
          echo "Package size: $PACKAGE_SIZE bytes"
          if [ $PACKAGE_SIZE -gt 5242880 ]; then  # 5MB limit
            echo "Package size exceeds 5MB limit"
            exit 1
          fi

  publish:
    name: Publish to NPM
    runs-on: ubuntu-latest
    needs: [test, benchmark, package-size]
    if: github.ref == 'refs/heads/main' && github.event_name == 'push'

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '18'
          cache: 'npm'
          registry-url: 'https://registry.npmjs.org'

      - name: Install dependencies
        run: npm ci

      - name: Build package
        run: npm run build

      - name: Generate docs
        run: npm run docs

      - name: Publish to NPM
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}

      - name: Create GitHub Release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: v${{ env.PACKAGE_VERSION }}
          release_name: Release v${{ env.PACKAGE_VERSION }}
          draft: false
          prerelease: false
```

### Test Configuration (jest.config.js)

```javascript
module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'node',
  roots: ['<rootDir>/src', '<rootDir>/tests'],
  testMatch: [
    '**/__tests__/**/*.ts',
    '**/?(*.)+(spec|test).ts'
  ],
  collectCoverageFrom: [
    'src/**/*.ts',
    '!src/**/*.d.ts',
    '!src/cli/**/*.ts'  // Exclude CLI from coverage
  ],
  coverageThreshold: {
    global: {
      branches: 80,
      functions: 80,
      lines: 80,
      statements: 80
    }
  },
  setupFilesAfterEnv: ['<rootDir>/tests/setup.ts'],
  testTimeout: 30000
};
```

## 7. Documentation & Examples

### README.md Template

```markdown
# @myorg/add-solver

> Sublinear Time Sparse Linear System Solver

[![npm version](https://badge.fury.io/js/%40myorg%2Fadd-solver.svg)](https://npmjs.com/package/@myorg/add-solver)
[![Build Status](https://github.com/myorg/add-solver/workflows/CI/badge.svg)](https://github.com/myorg/add-solver/actions)
[![Coverage Status](https://coveralls.io/repos/github/myorg/add-solver/badge.svg)](https://coveralls.io/github/myorg/add-solver)

## Quick Start

```bash
# Install globally for CLI usage
npm install -g @myorg/add-solver

# Or use with npx
npx @myorg/add-solver solve --matrix system.mtx --vector b.csv

# Install as library dependency
npm install @myorg/add-solver
```

## CLI Usage

```bash
# Solve linear system
add-solver solve --matrix A.mtx --vector b.csv --output x.json

# Start server mode
add-solver serve --port 8080 --cors

# Run benchmarks
add-solver benchmark --size 10000 --methods all
```

## Library Usage

```typescript
import { Solver, MatrixLoader } from '@myorg/add-solver';

const matrix = await MatrixLoader.load('system.mtx');
const solver = await Solver.create({ method: 'adaptive' });
const solution = await solver.solve(vector);
```

## Features

- 🚀 Sublinear time complexity for sparse systems
- 🎯 Adaptive algorithm selection
- 📊 Real-time progress streaming
- 🔧 Multiple input/output formats
- 🌐 HTTP server mode
- 📦 TypeScript support
- 🧪 Comprehensive testing

## Documentation

- [API Reference](https://myorg.github.io/add-solver/api)
- [Examples](./examples)
- [CLI Guide](./docs/cli.md)
- [Server API](./docs/server.md)
```

## 8. Migration and Compatibility

### Migration Guide (docs/migration.md)

```markdown
# Migration Guide

## From v0.x to v1.x

### Breaking Changes

1. **Package Name**: `sparse-solver` → `@myorg/add-solver`
2. **Import Paths**: All imports now from main package
3. **API Changes**: Async/await required for all operations

### Before (v0.x)
```javascript
const solver = require('sparse-solver');
const result = solver.solve(matrix, vector);
```

### After (v1.x)
```typescript
import { Solver } from '@myorg/add-solver';
const solver = await Solver.create();
const result = await solver.solve(vector);
```

### Migration Script

```bash
# Automated migration tool
npx @myorg/add-solver migrate --from v0.x --to v1.x --path ./src
```
```

This comprehensive plan provides a professional, feature-rich CLI interface and npm package that offers excellent developer experience, robust testing, and production-ready deployment capabilities.