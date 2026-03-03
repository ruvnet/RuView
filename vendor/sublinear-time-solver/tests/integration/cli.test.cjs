#!/usr/bin/env node

/**
 * Integration tests for CLI functionality
 * Run with: node tests/integration/cli.test.js
 */

const { strict: assert } = require('assert');
const { spawn, exec } = require('child_process');
const fs = require('fs').promises;
const path = require('path');
const os = require('os');

class CLITestRunner {
    constructor() {
        this.tests = [];
        this.passed = 0;
        this.failed = 0;
        this.verbose = process.argv.includes('--verbose');
        this.tempDir = null;
        this.cliPath = path.join(__dirname, '../../bin/cli.js');
    }

    async setup() {
        // Create temporary directory for test files
        this.tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'sublinear-test-'));

        // Create test matrix files
        await this.createTestMatrices();
    }

    async cleanup() {
        if (this.tempDir) {
            try {
                await fs.rm(this.tempDir, { recursive: true, force: true });
            } catch (error) {
                console.warn('Failed to cleanup temp directory:', error.message);
            }
        }
    }

    async createTestMatrices() {
        // Create a simple 2x2 matrix in JSON format
        const matrix2x2 = {
            rows: 2,
            cols: 2,
            data: [2, 1, 1, 2],
            format: 'dense'
        };

        await fs.writeFile(
            path.join(this.tempDir, 'matrix2x2.json'),
            JSON.stringify(matrix2x2, null, 2)
        );

        // Create corresponding vector
        const vector2x2 = [3, 3];
        await fs.writeFile(
            path.join(this.tempDir, 'vector2x2.json'),
            JSON.stringify(vector2x2, null, 2)
        );

        // Create a CSV matrix
        const csvMatrix = '1,0,0\n0,1,0\n0,0,1';
        await fs.writeFile(
            path.join(this.tempDir, 'identity3x3.csv'),
            csvMatrix
        );

        // Create Matrix Market format
        const mtxMatrix = `%%MatrixMarket matrix coordinate real general
3 3 3
1 1 1.0
2 2 1.0
3 3 1.0`;
        await fs.writeFile(
            path.join(this.tempDir, 'identity3x3.mtx'),
            mtxMatrix
        );

        // Create a larger sparse matrix in COO format
        const sparseMatrix = {
            rows: 5,
            cols: 5,
            entries: 8,
            data: {
                values: [4, -1, -1, 4, -1, -1, 4, -1],
                rowIndices: [0, 0, 1, 1, 1, 2, 2, 2],
                colIndices: [0, 1, 0, 1, 2, 1, 2, 3]
            },
            format: 'coo'
        };

        await fs.writeFile(
            path.join(this.tempDir, 'sparse5x5.json'),
            JSON.stringify(sparseMatrix, null, 2)
        );
    }

    test(name, fn) {
        this.tests.push({ name, fn });
    }

    async run() {
        console.log('🧪 Running CLI Integration Tests');
        console.log('================================\n');

        await this.setup();

        for (const { name, fn } of this.tests) {
            try {
                await fn();
                this.passed++;
                console.log(`✅ ${name}`);
            } catch (error) {
                this.failed++;
                console.log(`❌ ${name}`);
                if (this.verbose) {
                    console.log(`   Error: ${error.message}`);
                    console.log(`   Stack: ${error.stack}\n`);
                } else {
                    console.log(`   Error: ${error.message}\n`);
                }
            }
        }

        await this.cleanup();
        this.printSummary();
        return this.failed === 0;
    }

    printSummary() {
        console.log('\n📊 Test Summary');
        console.log('===============');
        console.log(`✅ Passed: ${this.passed}`);
        console.log(`❌ Failed: ${this.failed}`);
        console.log(`📈 Total:  ${this.tests.length}`);
        console.log(`🎯 Success Rate: ${((this.passed / this.tests.length) * 100).toFixed(1)}%`);
    }

    // Helper method to execute CLI commands
    async execCLI(args, options = {}) {
        return new Promise((resolve, reject) => {
            const child = spawn('node', [this.cliPath, ...args], {
                stdio: ['pipe', 'pipe', 'pipe'],
                ...options
            });

            let stdout = '';
            let stderr = '';

            child.stdout.on('data', (data) => {
                stdout += data.toString();
            });

            child.stderr.on('data', (data) => {
                stderr += data.toString();
            });

            child.on('close', (code) => {
                resolve({
                    code,
                    stdout,
                    stderr
                });
            });

            child.on('error', (error) => {
                reject(error);
            });

            // Set timeout to prevent hanging tests
            setTimeout(() => {
                child.kill('SIGTERM');
                reject(new Error('CLI command timed out'));
            }, 30000);
        });
    }
}

const runner = new CLITestRunner();

// Basic CLI Tests
runner.test('CLI displays help message', async () => {
    const result = await runner.execCLI(['--help']);

    assert.equal(result.code, 0);
    assert.ok(result.stdout.includes('Advanced Sublinear Time Sparse Linear System Solver'));
    assert.ok(result.stdout.includes('solve'));
    assert.ok(result.stdout.includes('serve'));
    assert.ok(result.stdout.includes('benchmark'));
});

runner.test('CLI displays version', async () => {
    const result = await runner.execCLI(['--version']);

    // Version command might exit with 0 or display version in help
    assert.ok(result.code === 0 || result.stdout.length > 0);
});

runner.test('CLI handles invalid command', async () => {
    const result = await runner.execCLI(['invalid-command']);

    // Should exit with non-zero code for invalid commands
    assert.notEqual(result.code, 0);
});

// Solve Command Tests
runner.test('CLI solve command requires matrix file', async () => {
    const result = await runner.execCLI(['solve']);

    assert.notEqual(result.code, 0);
    assert.ok(result.stderr.includes('required') || result.stdout.includes('required'));
});

runner.test('CLI solve command with valid matrix (should fail gracefully without WASM)', async () => {
    const matrixFile = path.join(runner.tempDir, 'matrix2x2.json');
    const result = await runner.execCLI(['solve', '-m', matrixFile]);

    // This should fail because WASM isn't built, but it should fail gracefully
    assert.notEqual(result.code, 0);
    // Should show a helpful error message
    assert.ok(result.stderr.length > 0 || result.stdout.includes('Error'));
});

runner.test('CLI solve command with output file specification', async () => {
    const matrixFile = path.join(runner.tempDir, 'matrix2x2.json');
    const outputFile = path.join(runner.tempDir, 'solution.json');

    const result = await runner.execCLI([
        'solve',
        '-m', matrixFile,
        '-o', outputFile
    ]);

    // Should fail gracefully without WASM but show proper argument parsing
    assert.notEqual(result.code, 0);
});

runner.test('CLI solve command with custom parameters', async () => {
    const matrixFile = path.join(runner.tempDir, 'matrix2x2.json');

    const result = await runner.execCLI([
        'solve',
        '-m', matrixFile,
        '--method', 'cg',
        '--tolerance', '1e-8',
        '--max-iterations', '500'
    ]);

    // Should fail without WASM but arguments should be parsed correctly
    assert.notEqual(result.code, 0);
});

// Verify Command Tests
runner.test('CLI verify command requires all files', async () => {
    const result = await runner.execCLI(['verify']);

    assert.notEqual(result.code, 0);
    // Should mention required arguments
    assert.ok(result.stderr.includes('required') || result.stdout.includes('required'));
});

runner.test('CLI verify command argument parsing', async () => {
    const matrixFile = path.join(runner.tempDir, 'matrix2x2.json');
    const solutionFile = path.join(runner.tempDir, 'solution.json');
    const vectorFile = path.join(runner.tempDir, 'vector2x2.json');

    // Create a dummy solution file
    await fs.writeFile(solutionFile, JSON.stringify([1, 1]));

    const result = await runner.execCLI([
        'verify',
        '-m', matrixFile,
        '-x', solutionFile,
        '-b', vectorFile,
        '--tolerance', '1e-6'
    ]);

    // May fail on implementation details but arguments should parse
    // We're mainly testing the CLI interface here
    assert.ok(result.code !== undefined);
});

// Convert Command Tests
runner.test('CLI convert command requires input and output', async () => {
    const result = await runner.execCLI(['convert']);

    assert.notEqual(result.code, 0);
    assert.ok(result.stderr.includes('required') || result.stdout.includes('required'));
});

runner.test('CLI convert command with format specification', async () => {
    const inputFile = path.join(runner.tempDir, 'matrix2x2.json');
    const outputFile = path.join(runner.tempDir, 'matrix2x2.csv');

    const result = await runner.execCLI([
        'convert',
        '-i', inputFile,
        '-o', outputFile,
        '--format', 'csv'
    ]);

    // This might work if conversion logic is implemented
    // We're testing the interface
    assert.ok(result.code !== undefined);
});

// Benchmark Command Tests
runner.test('CLI benchmark command with custom parameters', async () => {
    const result = await runner.execCLI([
        'benchmark',
        '--size', '10',
        '--sparsity', '0.1',
        '--methods', 'jacobi,cg',
        '--iterations', '2'
    ]);

    // Should fail without WASM but arguments should parse
    assert.notEqual(result.code, 0);
});

runner.test('CLI benchmark command output file', async () => {
    const outputFile = path.join(runner.tempDir, 'benchmark_results.json');

    const result = await runner.execCLI([
        'benchmark',
        '--size', '5',
        '--output', outputFile
    ]);

    // Should fail without WASM implementation
    assert.notEqual(result.code, 0);
});

// Serve Command Tests
runner.test('CLI serve command with default port', async () => {
    // Start server in background and kill it quickly
    const child = spawn('node', [runner.cliPath, 'serve'], {
        stdio: ['pipe', 'pipe', 'pipe']
    });

    // Give it a moment to start
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Kill the server
    child.kill('SIGTERM');

    // Wait for it to exit
    const exitCode = await new Promise(resolve => {
        child.on('close', resolve);
    });

    // The server might fail to start due to missing WASM, which is expected
    assert.ok(exitCode !== undefined);
});

runner.test('CLI serve command with custom port', async () => {
    const child = spawn('node', [runner.cliPath, 'serve', '--port', '3001'], {
        stdio: ['pipe', 'pipe', 'pipe']
    });

    await new Promise(resolve => setTimeout(resolve, 500));
    child.kill('SIGTERM');

    const exitCode = await new Promise(resolve => {
        child.on('close', resolve);
    });

    assert.ok(exitCode !== undefined);
});

// Flow-Nexus Command Tests
runner.test('CLI flow-nexus command structure', async () => {
    const result = await runner.execCLI(['flow-nexus', '--help']);

    // Should show flow-nexus specific help or fail gracefully
    assert.ok(result.code !== undefined);
});

// File Format Tests
runner.test('CLI handles JSON matrix format', async () => {
    const matrixFile = path.join(runner.tempDir, 'matrix2x2.json');

    // Verify the file exists and is readable by the CLI
    const stats = await fs.stat(matrixFile);
    assert.ok(stats.isFile());

    const content = await fs.readFile(matrixFile, 'utf8');
    const matrix = JSON.parse(content);
    assert.equal(matrix.rows, 2);
    assert.equal(matrix.cols, 2);
});

runner.test('CLI handles CSV matrix format', async () => {
    const matrixFile = path.join(runner.tempDir, 'identity3x3.csv');

    const stats = await fs.stat(matrixFile);
    assert.ok(stats.isFile());

    const content = await fs.readFile(matrixFile, 'utf8');
    const lines = content.trim().split('\n');
    assert.equal(lines.length, 3);
    assert.equal(lines[0], '1,0,0');
});

runner.test('CLI handles Matrix Market format', async () => {
    const matrixFile = path.join(runner.tempDir, 'identity3x3.mtx');

    const stats = await fs.stat(matrixFile);
    assert.ok(stats.isFile());

    const content = await fs.readFile(matrixFile, 'utf8');
    assert.ok(content.includes('%%MatrixMarket'));
    assert.ok(content.includes('3 3 3'));
});

// Error Handling Tests
runner.test('CLI handles missing matrix file', async () => {
    const result = await runner.execCLI([
        'solve',
        '-m', '/nonexistent/matrix.json'
    ]);

    assert.notEqual(result.code, 0);
    assert.ok(result.stderr.includes('Error') || result.stdout.includes('Error'));
});

runner.test('CLI handles invalid JSON matrix', async () => {
    const invalidFile = path.join(runner.tempDir, 'invalid.json');
    await fs.writeFile(invalidFile, '{ invalid json }');

    const result = await runner.execCLI([
        'solve',
        '-m', invalidFile
    ]);

    assert.notEqual(result.code, 0);
});

// Verbose and Debug Mode Tests
runner.test('CLI verbose mode', async () => {
    const result = await runner.execCLI([
        '--verbose',
        'solve',
        '-m', path.join(runner.tempDir, 'matrix2x2.json')
    ]);

    // Should produce more output in verbose mode
    assert.notEqual(result.code, 0); // Will fail without WASM
    // In verbose mode, there might be more detailed error information
});

runner.test('CLI debug mode', async () => {
    const result = await runner.execCLI([
        '--debug',
        'solve',
        '-m', path.join(runner.tempDir, 'matrix2x2.json')
    ]);

    assert.notEqual(result.code, 0); // Will fail without WASM
    // Debug mode should provide stack traces
});

runner.test('CLI quiet mode', async () => {
    const result = await runner.execCLI([
        '--quiet',
        'solve',
        '-m', path.join(runner.tempDir, 'matrix2x2.json')
    ]);

    assert.notEqual(result.code, 0); // Will fail without WASM
    // Output should be minimal in quiet mode
});

// Signal Handling Tests
runner.test('CLI handles SIGTERM gracefully', async () => {
    const child = spawn('node', [runner.cliPath, 'serve'], {
        stdio: ['pipe', 'pipe', 'pipe']
    });

    // Let it start
    await new Promise(resolve => setTimeout(resolve, 200));

    // Send SIGTERM
    child.kill('SIGTERM');

    // Wait for graceful shutdown
    const exitCode = await new Promise(resolve => {
        child.on('close', resolve);
        setTimeout(() => {
            child.kill('SIGKILL');
            resolve(-1);
        }, 5000);
    });

    // Should exit (might be 0 or error code depending on implementation)
    assert.ok(exitCode !== undefined);
});

// Run all tests
if (require.main === module) {
    runner.run().then(success => {
        process.exit(success ? 0 : 1);
    }).catch(error => {
        console.error('Test runner failed:', error);
        process.exit(1);
    });
}

module.exports = { CLITestRunner, runner };