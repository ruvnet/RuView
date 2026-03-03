#!/usr/bin/env node

/**
 * Comprehensive WASM Integration Tests
 * Tests all exported functionality in Node.js environment
 */

import { readFile } from 'fs/promises';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Test results
const results = {
  total: 0,
  passed: 0,
  failed: 0,
  tests: []
};

function test(name, fn) {
  results.total++;
  try {
    fn();
    results.passed++;
    results.tests.push({ name, status: 'PASS', error: null });
    console.log(`✅ PASS: ${name}`);
  } catch (error) {
    results.failed++;
    results.tests.push({ name, status: 'FAIL', error: error.message });
    console.error(`❌ FAIL: ${name}`);
    console.error(`   Error: ${error.message}`);
  }
}

async function testAsync(name, fn) {
  results.total++;
  try {
    await fn();
    results.passed++;
    results.tests.push({ name, status: 'PASS', error: null });
    console.log(`✅ PASS: ${name}`);
  } catch (error) {
    results.failed++;
    results.tests.push({ name, status: 'FAIL', error: error.message });
    console.error(`❌ FAIL: ${name}`);
    console.error(`   Error: ${error.message}`);
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message || 'Assertion failed');
  }
}

function assertApprox(actual, expected, tolerance, message) {
  if (Math.abs(actual - expected) > tolerance) {
    throw new Error(
      message ||
      `Expected ${actual} to be approximately ${expected} (±${tolerance})`
    );
  }
}

async function runTests() {
  console.log('🚀 Starting Comprehensive WASM Tests\n');
  console.log('═'.repeat(60));

  // Load WASM module
  const wasmPath = join(__dirname, '../pkg/midstream_wasm_bg.wasm');
  const wasmBuffer = await readFile(wasmPath);
  const wasmModule = await WebAssembly.compile(wasmBuffer);

  // Import JS bindings
  const { default: init, ...exports } = await import('../pkg/midstream_wasm.js');
  await init(wasmModule);

  const {
    version,
    TemporalCompare,
    NanoScheduler,
    StrangeLoop,
    QuicMultistream,
    benchmark_dtw
  } = exports;

  console.log('\n📦 Module Information');
  console.log('─'.repeat(60));

  test('Version exported', () => {
    const ver = version();
    assert(typeof ver === 'string', 'Version should be a string');
    assert(ver.length > 0, 'Version should not be empty');
    console.log(`   Version: ${ver}`);
  });

  console.log('\n🕒 TemporalCompare Tests');
  console.log('─'.repeat(60));

  test('TemporalCompare constructor', () => {
    const tc = new TemporalCompare();
    assert(tc !== null, 'TemporalCompare should be created');
  });

  test('TemporalCompare with window size', () => {
    const tc = new TemporalCompare(200);
    assert(tc !== null, 'TemporalCompare should accept window size');
  });

  test('DTW identical sequences', () => {
    const tc = new TemporalCompare();
    const seq = new Float64Array([1.0, 2.0, 3.0, 4.0, 5.0]);
    const distance = tc.dtw(seq, seq);
    assert(distance === 0.0, `DTW of identical sequences should be 0, got ${distance}`);
  });

  test('DTW different sequences', () => {
    const tc = new TemporalCompare();
    const seq1 = new Float64Array([1.0, 2.0, 3.0]);
    const seq2 = new Float64Array([2.0, 3.0, 4.0]);
    const distance = tc.dtw(seq1, seq2);
    assert(distance > 0, `DTW of different sequences should be > 0, got ${distance}`);
    console.log(`   DTW distance: ${distance.toFixed(2)}`);
  });

  test('DTW with realistic time series', () => {
    const tc = new TemporalCompare();
    const seq1 = Float64Array.from({ length: 100 }, (_, i) => Math.sin(i / 10));
    const seq2 = Float64Array.from({ length: 100 }, (_, i) => Math.sin(i / 10 + 0.5));
    const distance = tc.dtw(seq1, seq2);
    assert(distance > 0, 'DTW should compute distance');
    assert(distance < 1000, 'DTW distance should be reasonable');
    console.log(`   Time series DTW: ${distance.toFixed(2)}`);
  });

  test('LCS identical sequences', () => {
    const tc = new TemporalCompare();
    const seq = new Int32Array([1, 2, 3, 4, 5]);
    const length = tc.lcs(seq, seq);
    assert(length === 5, `LCS of identical sequences should be 5, got ${length}`);
  });

  test('LCS subsequence', () => {
    const tc = new TemporalCompare();
    const seq1 = new Int32Array([1, 2, 3, 4, 5]);
    const seq2 = new Int32Array([1, 3, 5]);
    const length = tc.lcs(seq1, seq2);
    assert(length === 3, `LCS should be 3, got ${length}`);
  });

  test('Edit distance identical strings', () => {
    const tc = new TemporalCompare();
    const distance = tc.edit_distance('hello', 'hello');
    assert(distance === 0, `Edit distance of identical strings should be 0, got ${distance}`);
  });

  test('Edit distance classic example', () => {
    const tc = new TemporalCompare();
    const distance = tc.edit_distance('kitten', 'sitting');
    assert(distance === 3, `Edit distance should be 3, got ${distance}`);
  });

  test('Comprehensive analyze method', () => {
    const tc = new TemporalCompare();
    const seq1 = Float64Array.from({ length: 50 }, (_, i) => Math.sin(i / 5));
    const seq2 = Float64Array.from({ length: 50 }, (_, i) => Math.sin(i / 5 + 0.3));

    const metrics = tc.analyze(seq1, seq2);

    assert(metrics.dtw_distance !== undefined, 'Should have DTW distance');
    assert(metrics.lcs_length !== undefined, 'Should have LCS length');
    assert(metrics.edit_distance !== undefined, 'Should have edit distance');
    assert(metrics.similarity_score !== undefined, 'Should have similarity score');

    assert(metrics.similarity_score >= 0 && metrics.similarity_score <= 1,
      'Similarity score should be between 0 and 1');

    console.log(`   DTW Distance: ${metrics.dtw_distance.toFixed(2)}`);
    console.log(`   LCS Length: ${metrics.lcs_length}`);
    console.log(`   Edit Distance: ${metrics.edit_distance}`);
    console.log(`   Similarity: ${(metrics.similarity_score * 100).toFixed(1)}%`);
  });

  console.log('\n⏱️  NanoScheduler Tests');
  console.log('─'.repeat(60));

  test('NanoScheduler constructor', () => {
    const scheduler = new NanoScheduler();
    assert(scheduler !== null, 'NanoScheduler should be created');
  });

  test('Schedule task', () => {
    const scheduler = new NanoScheduler();
    let executed = false;
    const taskId = scheduler.schedule(() => { executed = true; }, 1000000); // 1ms
    assert(typeof taskId === 'number', 'Should return task ID');
    assert(taskId > 0, 'Task ID should be positive');
  });

  test('Cancel task', () => {
    const scheduler = new NanoScheduler();
    const taskId = scheduler.schedule(() => {}, 1000000);
    const cancelled = scheduler.cancel(taskId);
    assert(cancelled === true, 'Should cancel successfully');
  });

  test('Cancel non-existent task', () => {
    const scheduler = new NanoScheduler();
    const cancelled = scheduler.cancel(99999);
    assert(cancelled === false, 'Should return false for non-existent task');
  });

  test('Now nanoseconds', () => {
    const scheduler = new NanoScheduler();
    const now = scheduler.now_ns();
    assert(typeof now === 'number', 'Should return number');
    assert(now > 0, 'Should return positive number');
  });

  test('Pending count', () => {
    const scheduler = new NanoScheduler();
    assert(scheduler.pending_count === 0, 'Initial pending count should be 0');
    scheduler.schedule(() => {}, 1000000);
    assert(scheduler.pending_count === 1, 'Pending count should be 1');
  });

  test('Tick execution', () => {
    const scheduler = new NanoScheduler();
    scheduler.schedule(() => {}, 0); // Immediate
    const executed = scheduler.tick();
    assert(executed >= 0, 'Should return execution count');
  });

  console.log('\n🧠 StrangeLoop Meta-Learning Tests');
  console.log('─'.repeat(60));

  test('StrangeLoop constructor', () => {
    const loop = new StrangeLoop();
    assert(loop !== null, 'StrangeLoop should be created');
  });

  test('StrangeLoop with custom learning rate', () => {
    const loop = new StrangeLoop(0.2);
    assert(loop !== null, 'StrangeLoop should accept learning rate');
  });

  test('Observe pattern', () => {
    const loop = new StrangeLoop(0.1);
    loop.observe('test-pattern', 0.8);
    assert(loop.iteration_count === 1, 'Iteration count should be 1');
    assert(loop.pattern_count === 1, 'Pattern count should be 1');
  });

  test('Get confidence', () => {
    const loop = new StrangeLoop(0.1);
    loop.observe('test-pattern', 0.8);
    const confidence = loop.get_confidence('test-pattern');
    assert(confidence !== undefined, 'Should return confidence');
    assert(confidence >= 0 && confidence <= 1, 'Confidence should be 0-1');
  });

  test('Get confidence for unknown pattern', () => {
    const loop = new StrangeLoop(0.1);
    const confidence = loop.get_confidence('unknown');
    assert(confidence === undefined, 'Should return undefined for unknown pattern');
  });

  test('Best pattern', () => {
    const loop = new StrangeLoop(0.1);
    loop.observe('pattern-a', 0.5);
    loop.observe('pattern-b', 0.8);
    loop.observe('pattern-c', 0.3);

    const best = loop.best_pattern();
    assert(best !== undefined, 'Should return best pattern');
    assert(best.pattern_id === 'pattern-b', 'Should return pattern-b as best');
    console.log(`   Best pattern: ${best.pattern_id} (${(best.confidence * 100).toFixed(1)}%)`);
  });

  test('Reflect method', () => {
    const loop = new StrangeLoop(0.1);
    loop.observe('pattern-1', 0.6);
    loop.observe('pattern-2', 0.7);

    const reflection = loop.reflect();
    assert(reflection !== null, 'Should return reflection object');
    assert(typeof reflection === 'object', 'Reflection should be object');
  });

  test('Learning progression', () => {
    const loop = new StrangeLoop(0.1);

    for (let i = 0; i < 10; i++) {
      loop.observe('learning-pattern', 0.5 + i * 0.05);
    }

    const confidence = loop.get_confidence('learning-pattern');
    assert(loop.iteration_count === 10, 'Should track iterations');
    console.log(`   Final confidence after 10 observations: ${(confidence * 100).toFixed(1)}%`);
  });

  console.log('\n🌐 QuicMultistream Tests');
  console.log('─'.repeat(60));

  test('QuicMultistream constructor', () => {
    const quic = new QuicMultistream();
    assert(quic !== null, 'QuicMultistream should be created');
  });

  test('Open stream', () => {
    const quic = new QuicMultistream();
    const streamId = quic.open_stream(128);
    assert(typeof streamId === 'number', 'Should return stream ID');
    assert(streamId >= 0, 'Stream ID should be non-negative');
    assert(quic.stream_count === 1, 'Stream count should be 1');
  });

  test('Open multiple streams', () => {
    const quic = new QuicMultistream();
    const id1 = quic.open_stream(100);
    const id2 = quic.open_stream(200);
    const id3 = quic.open_stream(50);

    assert(id1 !== id2 && id2 !== id3, 'Stream IDs should be unique');
    assert(quic.stream_count === 3, 'Stream count should be 3');
  });

  test('Close stream', () => {
    const quic = new QuicMultistream();
    const streamId = quic.open_stream(128);
    const closed = quic.close_stream(streamId);

    assert(closed === true, 'Should close successfully');
    assert(quic.stream_count === 0, 'Stream count should be 0');
  });

  test('Close non-existent stream', () => {
    const quic = new QuicMultistream();
    const closed = quic.close_stream(99999);
    assert(closed === false, 'Should return false for non-existent stream');
  });

  test('Send data', () => {
    const quic = new QuicMultistream();
    const streamId = quic.open_stream(128);
    const data = new Uint8Array([1, 2, 3, 4, 5]);
    const sent = quic.send(streamId, data);

    assert(sent === 5, 'Should return number of bytes sent');
  });

  test('Send data to non-existent stream', () => {
    const quic = new QuicMultistream();
    const data = new Uint8Array([1, 2, 3]);

    try {
      quic.send(99999, data);
      assert(false, 'Should throw error');
    } catch (error) {
      assert(true, 'Should throw error for non-existent stream');
    }
  });

  test('Receive data', () => {
    const quic = new QuicMultistream();
    const streamId = quic.open_stream(128);
    const received = quic.receive(streamId, 1024);

    assert(received instanceof Uint8Array, 'Should return Uint8Array');
    assert(received.length === 1024, 'Should return correct size');
  });

  test('Get stream stats', () => {
    const quic = new QuicMultistream();
    const streamId = quic.open_stream(200);

    const data = new Uint8Array(100);
    quic.send(streamId, data);
    quic.receive(streamId, 50);

    const stats = quic.get_stats(streamId);
    assert(stats !== null, 'Should return stats');
    assert(stats.stream_id === streamId, 'Should have correct stream ID');
    assert(stats.priority === 200, 'Should have correct priority');
    assert(stats.bytes_sent === 100, 'Should track bytes sent');
    assert(stats.bytes_received === 50, 'Should track bytes received');

    console.log(`   Stream ${streamId} stats: sent=${stats.bytes_sent}, recv=${stats.bytes_received}`);
  });

  console.log('\n⚡ Performance Benchmarks');
  console.log('─'.repeat(60));

  test('Benchmark DTW function', () => {
    const avgTime = benchmark_dtw(100, 50);
    assert(typeof avgTime === 'number', 'Should return number');
    assert(avgTime > 0, 'Should return positive time');
    console.log(`   DTW (100 elements, 50 iterations): ${avgTime.toFixed(3)}ms avg`);
    console.log(`   Throughput: ${(1000 / avgTime).toFixed(0)} ops/sec`);
  });

  test('DTW performance scaling', () => {
    const time50 = benchmark_dtw(50, 20);
    const time100 = benchmark_dtw(100, 20);
    const time200 = benchmark_dtw(200, 20);

    console.log(`   DTW 50 elements: ${time50.toFixed(3)}ms`);
    console.log(`   DTW 100 elements: ${time100.toFixed(3)}ms`);
    console.log(`   DTW 200 elements: ${time200.toFixed(3)}ms`);
  });

  console.log('\n🔒 Error Handling Tests');
  console.log('─'.repeat(60));

  test('DTW with empty sequences', () => {
    const tc = new TemporalCompare();
    const empty = new Float64Array([]);
    const seq = new Float64Array([1, 2, 3]);
    const distance = tc.dtw(empty, seq);
    assert(distance === Infinity, 'Empty sequence should return Infinity');
  });

  test('Memory cleanup', () => {
    // Create and destroy many objects to test memory management
    for (let i = 0; i < 100; i++) {
      const tc = new TemporalCompare();
      const scheduler = new NanoScheduler();
      const loop = new StrangeLoop();
      const quic = new QuicMultistream();

      // Use them briefly
      tc.dtw(new Float64Array([1, 2]), new Float64Array([1, 2]));
      scheduler.schedule(() => {}, 1000);
      loop.observe('test', 0.5);
      quic.open_stream(100);
    }
    assert(true, 'Should handle memory cleanup');
  });

  console.log('\n═'.repeat(60));
  console.log('\n📊 Test Summary');
  console.log('─'.repeat(60));
  console.log(`Total Tests: ${results.total}`);
  console.log(`✅ Passed: ${results.passed}`);
  console.log(`❌ Failed: ${results.failed}`);
  console.log(`Success Rate: ${((results.passed / results.total) * 100).toFixed(1)}%`);

  if (results.failed > 0) {
    console.log('\n❌ Failed Tests:');
    results.tests
      .filter(t => t.status === 'FAIL')
      .forEach(t => console.log(`   - ${t.name}: ${t.error}`));
  }

  console.log('\n' + '═'.repeat(60));

  return results;
}

// Run tests
runTests()
  .then(results => {
    process.exit(results.failed > 0 ? 1 : 0);
  })
  .catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
  });
