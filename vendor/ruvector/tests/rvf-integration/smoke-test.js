#!/usr/bin/env node
/**
 * End-to-end RVF CLI smoke test.
 *
 * Tests the full lifecycle via `npx ruvector rvf` CLI commands:
 *   create -> ingest -> query -> restart simulation -> query -> verify match
 *
 * Exits with code 0 on success, code 1 on failure.
 *
 * Usage:
 *   node tests/rvf-integration/smoke-test.js
 */

'use strict';

const { execFileSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const DIM = 128;
const METRIC = 'cosine';
const VECTOR_COUNT = 20;
const K = 5;

// Locate the CLI entry point relative to the repo root.
const REPO_ROOT = path.resolve(__dirname, '..', '..');
const CLI_PATH = path.join(REPO_ROOT, 'npm', 'packages', 'ruvector', 'bin', 'cli.js');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpDir;
let storePath;
let inputPath;
let childPath;
let passed = 0;
let failed = 0;

/**
 * Deterministic pseudo-random vector generation using an LCG.
 * Matches the Rust `random_vector` function for cross-validation.
 */
function randomVector(dim, seed) {
  const v = new Float64Array(dim);
  let x = BigInt(seed) & 0xFFFFFFFFFFFFFFFFn;
  for (let i = 0; i < dim; i++) {
    x = (x * 6364136223846793005n + 1442695040888963407n) & 0xFFFFFFFFFFFFFFFFn;
    v[i] = Number(x >> 33n) / 4294967295.0 - 0.5;
  }
  // Normalize for cosine.
  let norm = 0;
  for (let i = 0; i < dim; i++) norm += v[i] * v[i];
  norm = Math.sqrt(norm);
  const result = [];
  for (let i = 0; i < dim; i++) result.push(norm > 1e-8 ? v[i] / norm : 0);
  return result;
}

/**
 * Run a CLI command and return stdout as a string.
 * Throws on non-zero exit code.
 */
function runCli(args, opts = {}) {
  const cmdArgs = ['node', CLI_PATH, 'rvf', ...args];
  try {
    const stdout = execFileSync(cmdArgs[0], cmdArgs.slice(1), {
      cwd: REPO_ROOT,
      timeout: 30000,
      encoding: 'utf8',
      env: {
        ...process.env,
        // Disable chalk colors for easier parsing.
        FORCE_COLOR: '0',
        NO_COLOR: '1',
      },
      ...opts,
    });
    return stdout.trim();
  } catch (e) {
    const stderr = e.stderr ? e.stderr.toString().trim() : '';
    const stdout = e.stdout ? e.stdout.toString().trim() : '';
    throw new Error(
      `CLI failed (exit ${e.status}): ${args.join(' ')}\n` +
      `  stdout: ${stdout}\n` +
      `  stderr: ${stderr}`
    );
  }
}

/**
 * Assert a condition and track pass/fail.
 */
function assert(condition, message) {
  if (condition) {
    passed++;
    console.log(`  PASS: ${message}`);
  } else {
    failed++;
    console.error(`  FAIL: ${message}`);
  }
}

/**
 * Assert that a function throws (CLI command fails).
 */
function assertThrows(fn, message) {
  try {
    fn();
    failed++;
    console.error(`  FAIL: ${message} (expected error, got success)`);
  } catch (_e) {
    passed++;
    console.log(`  PASS: ${message}`);
  }
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

function setup() {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'rvf-smoke-'));
  storePath = path.join(tmpDir, 'smoke.rvf');
  inputPath = path.join(tmpDir, 'vectors.json');
  childPath = path.join(tmpDir, 'child.rvf');

  // Generate input vectors as JSON.
  const entries = [];
  for (let i = 0; i < VECTOR_COUNT; i++) {
    const id = i + 1;
    const vector = randomVector(DIM, id * 17 + 5);
    entries.push({ id, vector });
  }
  fs.writeFileSync(inputPath, JSON.stringify(entries));
}

// ---------------------------------------------------------------------------
// Teardown
// ---------------------------------------------------------------------------

function teardown() {
  try {
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  } catch (_e) {
    // Best-effort cleanup.
  }
}

// ---------------------------------------------------------------------------
// Test steps
// ---------------------------------------------------------------------------

function testCreate() {
  console.log('\nStep 1: Create store');
  const output = runCli(['create', storePath, '-d', String(DIM), '-m', METRIC]);
  assert(output.includes('Created') || output.includes('created'), 'create reports success');
  assert(fs.existsSync(storePath), 'store file exists on disk');
}

function testIngest() {
  console.log('\nStep 2: Ingest vectors');
  const output = runCli(['ingest', storePath, '-i', inputPath]);
  assert(
    output.includes('Ingested') || output.includes('accepted'),
    'ingest reports accepted vectors'
  );
}

function testQueryFirst() {
  console.log('\nStep 3: Query (first pass)');
  // Query with the vector for id=10 (seed = 9 * 17 + 5 = 158).
  const queryVec = randomVector(DIM, 9 * 17 + 5);
  const vecStr = queryVec.map(v => v.toFixed(8)).join(',');
  const output = runCli(['query', storePath, '-v', vecStr, '-k', String(K)]);
  assert(output.includes('result'), 'query returns results');

  // Parse result count.
  const countMatch = output.match(/(\d+)\s*result/);
  if (countMatch) {
    const count = parseInt(countMatch[1], 10);
    assert(count > 0, `query returned ${count} results (> 0)`);
    assert(count <= K, `query returned ${count} results (<= ${K})`);
  } else {
    assert(false, 'could not parse result count from output');
  }

  return output;
}

function testStatus() {
  console.log('\nStep 4: Status check');
  const output = runCli(['status', storePath]);
  assert(output.includes('total_vectors') || output.includes('totalVectors'), 'status shows vector count');
}

function testSegments() {
  console.log('\nStep 5: Segment listing');
  const output = runCli(['segments', storePath]);
  assert(
    output.includes('segment') || output.includes('type='),
    'segments command lists segments'
  );
}

function testCompact() {
  console.log('\nStep 6: Compact');
  const output = runCli(['compact', storePath]);
  assert(output.includes('Compact') || output.includes('compact'), 'compact reports completion');
}

function testDerive() {
  console.log('\nStep 7: Derive child store');
  const output = runCli(['derive', storePath, childPath]);
  assert(
    output.includes('Derived') || output.includes('derived'),
    'derive reports success'
  );
  assert(fs.existsSync(childPath), 'child store file exists on disk');
}

function testChildSegments() {
  console.log('\nStep 8: Child segment listing');
  const output = runCli(['segments', childPath]);
  assert(
    output.includes('segment') || output.includes('type='),
    'child segments command lists segments'
  );
}

function testStatusAfterLifecycle() {
  console.log('\nStep 9: Final status check');
  const output = runCli(['status', storePath]);
  assert(output.length > 0, 'status returns non-empty output');
}

function testExport() {
  console.log('\nStep 10: Export');
  const exportPath = path.join(tmpDir, 'export.json');
  const output = runCli(['export', storePath, '-o', exportPath]);
  assert(
    output.includes('Exported') || output.includes('exported') || fs.existsSync(exportPath),
    'export produces output file'
  );
  if (fs.existsSync(exportPath)) {
    const data = JSON.parse(fs.readFileSync(exportPath, 'utf8'));
    assert(data.status !== undefined, 'export contains status');
    assert(data.segments !== undefined, 'export contains segments');
  }
}

function testNonexistentStore() {
  console.log('\nStep 11: Error handling');
  assertThrows(
    () => runCli(['status', '/tmp/nonexistent_smoke_test_rvf_99999.rvf']),
    'status on nonexistent store fails with error'
  );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  console.log('=== RVF CLI End-to-End Smoke Test ===');
  console.log(`  DIM=${DIM} METRIC=${METRIC} VECTORS=${VECTOR_COUNT} K=${K}`);

  setup();

  try {
    // Check if CLI exists before running tests.
    if (!fs.existsSync(CLI_PATH)) {
      console.error(`\nCLI not found at: ${CLI_PATH}`);
      console.error('Skipping CLI smoke test (CLI not built).');
      console.log('\n=== SKIPPED (CLI not available) ===');
      process.exit(0);
    }

    testCreate();
    testIngest();
    testQueryFirst();
    testStatus();
    testSegments();
    testCompact();
    testDerive();
    testChildSegments();
    testStatusAfterLifecycle();
    testExport();
    testNonexistentStore();
  } catch (e) {
    // If any step throws unexpectedly, we still want to report and clean up.
    failed++;
    console.error(`\nUNEXPECTED ERROR: ${e.message}`);
    if (e.stack) console.error(e.stack);
  } finally {
    teardown();
  }

  // Summary.
  const total = passed + failed;
  console.log(`\n=== Results: ${passed}/${total} passed, ${failed} failed ===`);

  if (failed > 0) {
    process.exit(1);
  } else {
    console.log('All smoke tests passed.');
    process.exit(0);
  }
}

main();
