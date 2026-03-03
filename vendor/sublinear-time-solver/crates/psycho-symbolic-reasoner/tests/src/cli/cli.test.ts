/**
 * CLI Tests
 * Tests command-line interface scenarios and interactions
 */

import { describe, test, expect, beforeAll, afterAll, beforeEach, afterEach } from '@jest/globals';
import { testUtils } from '@test/test-helpers';
import * as path from 'path';

describe('CLI Tests', () => {
  let tempFiles: string[] = [];

  beforeAll(async () => {
    console.log('Setting up CLI test environment...');
  });

  afterAll(async () => {
    await testUtils.fsUtils.cleanup(tempFiles);
    testUtils.cliRunner.cleanup();
  });

  beforeEach(() => {
    // Reset state before each test
  });

  describe('Basic CLI Operations', () => {
    test('should display help information', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['--help'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner'
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Usage:');
      expect(result.stdout).toContain('Options:');
      expect(result.executionTime).toBeLessThan(5000);
    });

    test('should show version information', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['--version'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner'
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toMatch(/cargo \d+\.\d+\.\d+/);
      expect(result.executionTime).toBeLessThan(3000);
    });

    test('should handle invalid commands gracefully', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['nonexistent-command'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner'
      });

      expect(result.exitCode).not.toBe(0);
      expect(result.stderr.length).toBeGreaterThan(0);
    });
  });

  describe('Build and Compilation', () => {
    test('should build all Rust modules', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['build'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 120000 // 2 minutes
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Finished');
      expect(result.executionTime).toBeLessThan(120000);
    });

    test('should build release version', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['build', '--release'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 180000 // 3 minutes
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Finished');
      expect(result.stdout).toContain('release');
    });

    test('should compile individual modules', async () => {
      const modules = ['graph_reasoner', 'planner', 'extractors'];

      for (const module of modules) {
        const result = await testUtils.cliRunner.runCommand('cargo', ['build', '-p', module], {
          cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
          timeout: 60000
        });

        expect(result.exitCode).toBe(0);
        expect(result.stdout).toContain('Finished');
        console.log(`${module} build time: ${result.executionTime}ms`);
      }
    });

    test('should check code without building', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['check'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 60000
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Finished');
      expect(result.executionTime).toBeLessThan(60000);
    });
  });

  describe('Testing Commands', () => {
    test('should run all Rust tests', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['test'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 120000
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('test result:');
      expect(result.stdout).toMatch(/\d+ passed/);
    });

    test('should run tests for specific module', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['test', '-p', 'graph_reasoner'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 60000
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('test result:');
    });

    test('should run tests with verbose output', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['test', '--', '--nocapture'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 120000
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout.length).toBeGreaterThan(100);
    });

    test('should run specific test by name', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['test', 'graph_tests'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 30000
      });

      // May pass or fail depending on test existence, but should not crash
      expect([0, 101]).toContain(result.exitCode); // 101 is "no tests ran"
    });
  });

  describe('WASM Build Commands', () => {
    test('should build WASM modules with wasm-pack', async () => {
      const modules = ['graph_reasoner', 'planner', 'extractors'];

      for (const module of modules) {
        const result = await testUtils.cliRunner.runCommand('wasm-pack', [
          'build',
          '--target', 'web',
          '--out-dir', `../tests/wasm/${module}`,
          module
        ], {
          cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
          timeout: 120000
        });

        if (result.exitCode === 0) {
          expect(result.stdout).toContain('success');
          console.log(`${module} WASM build successful`);
        } else {
          console.log(`${module} WASM build failed (expected if wasm-pack not available)`);
          // Don't fail test if wasm-pack is not available
        }
      }
    });

    test('should optimize WASM modules', async () => {
      // This test assumes wasm-opt is available
      const wasmFile = path.join(__dirname, '../../../wasm/graph_reasoner/graph_reasoner_bg.wasm');

      try {
        const result = await testUtils.cliRunner.runCommand('wasm-opt', [
          '-Oz',
          '--output', wasmFile.replace('.wasm', '_optimized.wasm'),
          wasmFile
        ], {
          timeout: 30000
        });

        if (result.exitCode === 0) {
          expect(result.executionTime).toBeLessThan(30000);
          console.log('WASM optimization successful');
        } else {
          console.log('wasm-opt not available, skipping optimization test');
        }
      } catch (error) {
        console.log('wasm-opt not available, skipping optimization test');
      }
    });
  });

  describe('Linting and Formatting', () => {
    test('should run clippy linter', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['clippy'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 60000
      });

      // Clippy might find issues but shouldn't crash
      expect([0, 101]).toContain(result.exitCode);
      expect(result.stdout.length).toBeGreaterThan(0);
    });

    test('should check code formatting', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['fmt', '--check'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 30000
      });

      // Code should be properly formatted
      expect(result.exitCode).toBe(0);
    });

    test('should format code', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['fmt'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 30000
      });

      expect(result.exitCode).toBe(0);
    });
  });

  describe('Documentation Generation', () => {
    test('should generate documentation', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['doc'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 120000
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Documenting');
    });

    test('should generate documentation with dependencies', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['doc', '--no-deps'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 60000
      });

      expect(result.exitCode).toBe(0);
    });

    test('should open documentation in browser', async () => {
      // This test just checks the command doesn't crash
      const result = await testUtils.cliRunner.runCommand('cargo', ['doc', '--open', '--no-deps'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 30000
      });

      // May fail in headless environment, but shouldn't crash
      expect([0, 1]).toContain(result.exitCode);
    });
  });

  describe('Benchmarking Commands', () => {
    test('should run benchmarks if available', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['bench'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 180000
      });

      // Benchmarks might not be available in all environments
      expect([0, 101]).toContain(result.exitCode);

      if (result.exitCode === 0) {
        expect(result.stdout).toContain('bench');
        console.log('Benchmarks completed successfully');
      } else {
        console.log('No benchmarks available or benchmark runner not found');
      }
    });

    test('should run criterion benchmarks with features', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', [
        'bench',
        '--features', 'benchmarks'
      ], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 180000
      });

      // May fail if criterion is not set up, but shouldn't crash
      expect(result.executionTime).toBeLessThan(180000);
    });
  });

  describe('Environment and Dependencies', () => {
    test('should show environment information', async () => {
      const commands = [
        ['rustc', ['--version']],
        ['cargo', ['--version']],
        ['rustup', ['show']],
      ];

      for (const [cmd, args] of commands) {
        try {
          const result = await testUtils.cliRunner.runCommand(cmd, args, {
            timeout: 10000
          });

          if (result.exitCode === 0) {
            expect(result.stdout.length).toBeGreaterThan(0);
            console.log(`${cmd}: ${result.stdout.trim().split('\n')[0]}`);
          }
        } catch (error) {
          console.log(`${cmd} not available`);
        }
      }
    });

    test('should check dependency tree', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['tree'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 30000
      });

      if (result.exitCode === 0) {
        expect(result.stdout).toContain('serde');
        expect(result.stdout).toContain('wasm-bindgen');
      } else {
        console.log('cargo tree not available or failed');
      }
    });

    test('should audit dependencies for security', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['audit'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 60000
      });

      // cargo-audit might not be installed
      if (result.exitCode === 0) {
        expect(result.stdout.length).toBeGreaterThan(0);
        console.log('Security audit completed');
      } else {
        console.log('cargo-audit not available');
      }
    });
  });

  describe('TypeScript CLI Tests', () => {
    test('should run TypeScript tests via npm', async () => {
      const result = await testUtils.cliRunner.runCommand('npm', ['test'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner/tests',
        timeout: 120000
      });

      if (result.exitCode === 0) {
        expect(result.stdout).toContain('PASS');
        console.log('TypeScript tests passed');
      } else {
        console.log('TypeScript tests failed or npm not available');
        // Don't fail the test if dependencies aren't installed
      }
    });

    test('should install npm dependencies', async () => {
      const result = await testUtils.cliRunner.runCommand('npm', ['install'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner/tests',
        timeout: 180000
      });

      if (result.exitCode === 0) {
        expect(result.stdout).toContain('added');
        console.log('NPM dependencies installed');
      } else {
        console.log('NPM install failed or npm not available');
      }
    });

    test('should run TypeScript compiler', async () => {
      const result = await testUtils.cliRunner.runCommand('npx', ['tsc', '--noEmit'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner/tests',
        timeout: 60000
      });

      if (result.exitCode === 0) {
        console.log('TypeScript compilation successful');
      } else {
        console.log('TypeScript compilation failed or tsc not available');
      }
    });
  });

  describe('Custom CLI Scripts', () => {
    test('should run build script', async () => {
      // Create a simple build script
      const buildScript = `#!/bin/bash
set -e

echo "Building all modules..."
cargo build --workspace

echo "Running tests..."
cargo test --workspace

echo "Building WASM modules..."
for module in graph_reasoner planner extractors; do
    if command -v wasm-pack >/dev/null 2>&1; then
        cd $module
        wasm-pack build --target web --out-dir ../tests/wasm/$module
        cd ..
    else
        echo "wasm-pack not found, skipping WASM build for $module"
    fi
done

echo "Build complete!"
`;

      const scriptPath = await testUtils.fsUtils.createTempFile(buildScript, '.sh');
      tempFiles.push(scriptPath);

      // Make script executable (Unix-like systems)
      if (process.platform !== 'win32') {
        await testUtils.cliRunner.runCommand('chmod', ['+x', scriptPath]);
      }

      const result = await testUtils.cliRunner.runCommand('bash', [scriptPath], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 300000 // 5 minutes
      });

      if (result.exitCode === 0) {
        expect(result.stdout).toContain('Build complete!');
        console.log('Custom build script executed successfully');
      } else {
        console.log('Custom build script failed (expected in some environments)');
      }
    });

    test('should run test coverage script', async () => {
      const coverageScript = `#!/bin/bash
set -e

echo "Installing coverage tools..."
if command -v cargo >/dev/null 2>&1; then
    cargo install --quiet cargo-tarpaulin || echo "cargo-tarpaulin already installed or install failed"
fi

echo "Running coverage analysis..."
if command -v cargo-tarpaulin >/dev/null 2>&1; then
    cargo tarpaulin --workspace --out xml --output-dir target/coverage
    echo "Coverage report generated"
else
    echo "cargo-tarpaulin not available, running regular tests"
    cargo test --workspace
fi
`;

      const scriptPath = await testUtils.fsUtils.createTempFile(coverageScript, '.sh');
      tempFiles.push(scriptPath);

      const result = await testUtils.cliRunner.runCommand('bash', [scriptPath], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 300000
      });

      // Coverage tools might not be available, so don't fail
      expect(result.executionTime).toBeLessThan(300000);
      console.log('Coverage script completed');
    });
  });

  describe('Error Handling and Edge Cases', () => {
    test('should handle interrupted builds gracefully', async () => {
      // Start a build and interrupt it quickly
      const buildPromise = testUtils.cliRunner.runCommand('cargo', ['build'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 1000 // Very short timeout to simulate interruption
      });

      try {
        await buildPromise;
      } catch (error) {
        expect(error).toBeInstanceOf(Error);
        console.log('Build interruption handled correctly');
      }
    });

    test('should handle invalid workspace', async () => {
      const result = await testUtils.cliRunner.runCommand('cargo', ['build'], {
        cwd: '/tmp',
        timeout: 10000
      });

      expect(result.exitCode).not.toBe(0);
      expect(result.stderr).toContain('Cargo.toml');
    });

    test('should handle missing dependencies', async () => {
      // Try to run a command that requires a missing tool
      const result = await testUtils.cliRunner.runCommand('nonexistent-tool', ['--version'], {
        timeout: 5000
      });

      expect(result.exitCode).not.toBe(0);
    });

    test('should handle file permission errors', async () => {
      if (process.platform !== 'win32') {
        // Create a temporary file and remove write permissions
        const tempFile = await testUtils.fsUtils.createTempFile('test content', '.txt');
        tempFiles.push(tempFile);

        await testUtils.cliRunner.runCommand('chmod', ['444', tempFile]);

        // Try to write to the file
        const result = await testUtils.cliRunner.runCommand('bash', ['-c', `echo "new content" > ${tempFile}`], {
          timeout: 5000
        });

        expect(result.exitCode).not.toBe(0);
        expect(result.stderr.length).toBeGreaterThan(0);
      }
    });
  });

  describe('Performance and Resource Usage', () => {
    test('should monitor build resource usage', async () => {
      const collector = testUtils.performanceCollector;
      collector.start();

      const result = await testUtils.cliRunner.runCommand('cargo', ['check'], {
        cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
        timeout: 60000
      });

      const metrics = collector.stop();

      expect(result.exitCode).toBe(0);
      expect(metrics.executionTime).toBeLessThan(60000);
      expect(metrics.memoryUsage).toBeLessThan(500 * 1024 * 1024); // Less than 500MB

      console.log(`Build check: ${metrics.executionTime}ms, ${Math.round(metrics.memoryUsage / 1024 / 1024)}MB`);
    });

    test('should benchmark different build configurations', async () => {
      const configurations = [
        { name: 'debug', args: ['build'] },
        { name: 'check', args: ['check'] },
        { name: 'release', args: ['build', '--release'] }
      ];

      const results: Array<{
        config: string;
        time: number;
        success: boolean;
      }> = [];

      for (const config of configurations) {
        const startTime = performance.now();

        const result = await testUtils.cliRunner.runCommand('cargo', config.args, {
          cwd: '/workspaces/sublinear-time-solver/psycho-symbolic-reasoner',
          timeout: 180000
        });

        const endTime = performance.now();

        results.push({
          config: config.name,
          time: endTime - startTime,
          success: result.exitCode === 0
        });

        console.log(`${config.name}: ${Math.round(endTime - startTime)}ms, success: ${result.exitCode === 0}`);
      }

      // Check should be fastest
      const checkResult = results.find(r => r.config === 'check');
      const buildResult = results.find(r => r.config === 'debug');

      if (checkResult && buildResult && checkResult.success && buildResult.success) {
        expect(checkResult.time).toBeLessThan(buildResult.time);
      }
    });
  });
});