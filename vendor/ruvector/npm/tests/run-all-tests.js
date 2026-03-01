#!/usr/bin/env node

/**
 * Test runner for all npm packages
 * Runs unit tests, integration tests, and performance benchmarks
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

// ANSI colors
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  cyan: '\x1b[36m',
  blue: '\x1b[34m'
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function section(title) {
  console.log();
  log('='.repeat(70), 'cyan');
  log(`  ${title}`, 'bright');
  log('='.repeat(70), 'cyan');
  console.log();
}

async function runTest(name, testFile) {
  return new Promise((resolve) => {
    log(`Running: ${name}`, 'cyan');

    const test = spawn('node', ['--test', testFile], {
      cwd: path.dirname(testFile),
      stdio: 'inherit'
    });

    test.on('close', (code) => {
      if (code === 0) {
        log(`✓ ${name} passed`, 'green');
        resolve({ name, passed: true });
      } else {
        log(`✗ ${name} failed`, 'red');
        resolve({ name, passed: false, code });
      }
      console.log();
    });

    test.on('error', (error) => {
      log(`✗ ${name} errored: ${error.message}`, 'red');
      resolve({ name, passed: false, error: error.message });
      console.log();
    });
  });
}

async function main() {
  const args = process.argv.slice(2);
  const runPerf = args.includes('--perf');
  const runOnly = args.find(arg => arg.startsWith('--only='))?.split('=')[1];

  log('\n🧪 rUvector NPM Package Test Suite\n', 'bright');

  const results = [];

  // Define test suites
  const testSuites = [
    {
      category: 'unit',
      title: 'Unit Tests',
      tests: [
        { name: '@ruvector/core', file: './unit/core.test.js' },
        { name: '@ruvector/wasm', file: './unit/wasm.test.js' },
        { name: 'ruvector', file: './unit/ruvector.test.js' },
        { name: 'ruvector CLI', file: './unit/cli.test.js' }
      ]
    },
    {
      category: 'integration',
      title: 'Integration Tests',
      tests: [
        { name: 'Cross-package compatibility', file: './integration/cross-package.test.js' }
      ]
    }
  ];

  if (runPerf) {
    testSuites.push({
      category: 'performance',
      title: 'Performance Benchmarks',
      tests: [
        { name: 'Performance benchmarks', file: './performance/benchmarks.test.js' }
      ]
    });
  }

  // Run tests
  for (const suite of testSuites) {
    if (runOnly && suite.category !== runOnly) continue;

    section(suite.title);

    for (const test of suite.tests) {
      const testPath = path.join(__dirname, test.file);

      if (!fs.existsSync(testPath)) {
        log(`⚠ Skipping ${test.name} - file not found`, 'yellow');
        continue;
      }

      const result = await runTest(test.name, testPath);
      results.push({ ...result, category: suite.category });
    }
  }

  // Summary
  section('Test Summary');

  const passed = results.filter(r => r.passed).length;
  const failed = results.filter(r => !r.passed).length;
  const total = results.length;

  log(`Total: ${total}`, 'cyan');
  log(`Passed: ${passed}`, passed > 0 ? 'green' : 'reset');
  log(`Failed: ${failed}`, failed > 0 ? 'red' : 'reset');

  if (failed > 0) {
    console.log();
    log('Failed tests:', 'red');
    results.filter(r => !r.passed).forEach(r => {
      log(`  - ${r.name}`, 'red');
    });
  }

  console.log();

  // Generate report
  const report = {
    timestamp: new Date().toISOString(),
    summary: {
      total,
      passed,
      failed,
      passRate: ((passed / total) * 100).toFixed(1) + '%'
    },
    results: results.map(r => ({
      name: r.name,
      category: r.category,
      passed: r.passed,
      code: r.code,
      error: r.error
    }))
  };

  const reportPath = path.join(__dirname, 'test-results.json');
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  log(`Report saved to: ${reportPath}`, 'cyan');

  console.log();

  // Exit with appropriate code
  process.exit(failed > 0 ? 1 : 0);
}

main().catch(error => {
  console.error('Test runner error:', error);
  process.exit(1);
});
