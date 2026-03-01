/**
 * GNN Performance Benchmark Suite
 *
 * Tests performance of GNN operations and identifies bottlenecks
 */

const { performance } = require('perf_hooks');

// Try to load native GNN module directly
let gnnNative;
let gnnWrapper;

try {
  gnnNative = require('@ruvector/gnn');
  console.log('✅ @ruvector/gnn loaded');
} catch (e) {
  console.log('❌ @ruvector/gnn not available:', e.message);
}

// Benchmark utilities
function generateRandomVector(dim) {
  const arr = new Array(dim);
  for (let i = 0; i < dim; i++) {
    arr[i] = Math.random();
  }
  return arr;
}

function generateRandomFloat32(dim) {
  const arr = new Float32Array(dim);
  for (let i = 0; i < dim; i++) {
    arr[i] = Math.random();
  }
  return arr;
}

function benchmark(name, fn, iterations = 1000) {
  // Warmup
  for (let i = 0; i < 10; i++) fn();

  const times = [];
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    fn();
    times.push(performance.now() - start);
  }

  times.sort((a, b) => a - b);
  const avg = times.reduce((a, b) => a + b, 0) / times.length;
  const p50 = times[Math.floor(times.length * 0.5)];
  const p95 = times[Math.floor(times.length * 0.95)];
  const p99 = times[Math.floor(times.length * 0.99)];

  return { name, avg, p50, p95, p99, iterations };
}

function formatMs(ms) {
  if (ms < 0.001) return `${(ms * 1000000).toFixed(2)}ns`;
  if (ms < 1) return `${(ms * 1000).toFixed(2)}µs`;
  return `${ms.toFixed(2)}ms`;
}

function printResult(result) {
  console.log(`  ${result.name}:`);
  console.log(`    avg: ${formatMs(result.avg)} | p50: ${formatMs(result.p50)} | p95: ${formatMs(result.p95)} | p99: ${formatMs(result.p99)}`);
}

// Array conversion benchmarks
function benchmarkArrayConversion() {
  console.log('\n📊 Array Conversion Overhead Benchmarks');
  console.log('=========================================');

  const dims = [128, 256, 512, 768, 1024];

  for (const dim of dims) {
    console.log(`\n  Dimension: ${dim}`);

    const regularArray = generateRandomVector(dim);
    const float32Array = generateRandomFloat32(dim);

    // Test Array.from on Float32Array
    printResult(benchmark(`Array.from(Float32Array)`, () => {
      return Array.from(float32Array);
    }));

    // Test spread operator
    printResult(benchmark(`[...Float32Array]`, () => {
      return [...float32Array];
    }));

    // Test slice (for regular arrays - noop baseline)
    printResult(benchmark(`Array.slice() (baseline)`, () => {
      return regularArray.slice();
    }));

    // Test Float32Array.from
    printResult(benchmark(`Float32Array.from(Array)`, () => {
      return Float32Array.from(regularArray);
    }));

    // Test new Float32Array
    printResult(benchmark(`new Float32Array(Array)`, () => {
      return new Float32Array(regularArray);
    }));
  }
}

// GNN operation benchmarks
function benchmarkGnnOperations() {
  if (!gnnNative) {
    console.log('\n⚠️  Skipping GNN benchmarks - module not available');
    return;
  }

  console.log('\n📊 GNN Operation Benchmarks');
  console.log('===========================');

  const dims = [128, 256, 512];
  const candidateCounts = [100, 1000, 10000];

  for (const dim of dims) {
    for (const count of candidateCounts) {
      console.log(`\n  Dimension: ${dim}, Candidates: ${count}`);

      // Prepare data as regular arrays (user input)
      const queryArray = generateRandomVector(dim);
      const candidatesArray = Array.from({ length: count }, () => generateRandomVector(dim));

      // Prepare data as Float32Array (pre-converted for max performance)
      const queryFloat32 = new Float32Array(queryArray);
      const candidatesFloat32 = candidatesArray.map(arr => new Float32Array(arr));

      const iters = Math.min(100, Math.floor(10000 / count));

      // Measure Float32Array conversion overhead (Array -> Float32Array)
      const conversionOverheadResult = benchmark(`Array→Float32 conversion`, () => {
        const q = new Float32Array(queryArray);
        const c = candidatesArray.map(arr => new Float32Array(arr));
        return { q, c };
      }, iters);
      printResult(conversionOverheadResult);

      // Wrapped interface with regular arrays (tests full conversion + native)
      try {
        const wrappedArrayResult = benchmark(`Wrapped (from Array)`, () => {
          return gnnNative.differentiableSearch(queryArray, candidatesArray, 10, 1.0);
        }, iters);
        printResult(wrappedArrayResult);
      } catch (e) {
        console.log(`    Wrapped (from Array): Error - ${e.message}`);
      }

      // Wrapped interface with Float32Array (tests zero-copy path)
      try {
        const wrappedFloat32Result = benchmark(`Wrapped (from Float32)`, () => {
          return gnnNative.differentiableSearch(queryFloat32, candidatesFloat32, 10, 1.0);
        }, iters);
        printResult(wrappedFloat32Result);
      } catch (e) {
        console.log(`    Wrapped (from Float32): Error - ${e.message}`);
      }

      // Native direct with Float32Array (bypasses wrapper, max performance)
      try {
        const nativeResult = benchmark(`Native direct (Float32)`, () => {
          return gnnNative.nativeDifferentiableSearch(queryFloat32, candidatesFloat32, 10, 1.0);
        }, iters);
        printResult(nativeResult);
      } catch (e) {
        console.log(`    Native direct (Float32): Error - ${e.message}`);
      }

      console.log('');
    }
  }
}

// Batch operation benchmarks
function benchmarkBatchOperations() {
  if (!gnnNative) return;

  console.log('\n📊 Batch vs Sequential Benchmarks');
  console.log('==================================');

  const dim = 256;
  const batchSizes = [10, 50, 100];
  const candidateCount = 1000;

  const candidates = Array.from({ length: candidateCount }, () => generateRandomVector(dim));

  for (const batchSize of batchSizes) {
    console.log(`\n  Batch size: ${batchSize}, Candidates: ${candidateCount}`);

    const queries = Array.from({ length: batchSize }, () => generateRandomVector(dim));

    // Sequential search
    const sequentialResult = benchmark(`Sequential search`, () => {
      const results = [];
      for (const query of queries) {
        results.push(gnnNative.differentiableSearch(query, candidates, 10, 1.0));
      }
      return results;
    }, 10);
    printResult(sequentialResult);

    // Note: batch search would need to be implemented in native
    console.log(`    Batch search: Not implemented (potential ${batchSize}x improvement)`);
  }
}

// RuvectorLayer benchmarks
function benchmarkRuvectorLayer() {
  if (!gnnNative) return;

  console.log('\n📊 RuvectorLayer Benchmarks');
  console.log('===========================');

  const dims = [128, 256, 512];
  const neighborCounts = [5, 10, 20, 50];

  for (const dim of dims) {
    for (const neighborCount of neighborCounts) {
      console.log(`\n  Dimension: ${dim}, Neighbors: ${neighborCount}`);

      const layer = new gnnNative.RuvectorLayer(dim, dim, 4, 0.1);

      // Test with regular arrays (triggers conversion)
      const nodeArray = generateRandomVector(dim);
      const neighborsArray = Array.from({ length: neighborCount }, () => generateRandomVector(dim));
      const weightsArray = generateRandomVector(neighborCount);

      // Test with Float32Arrays (zero-copy)
      const nodeFloat32 = new Float32Array(nodeArray);
      const neighborsFloat32 = neighborsArray.map(arr => new Float32Array(arr));
      const weightsFloat32 = new Float32Array(weightsArray);

      try {
        const arrayResult = benchmark(`Layer forward (Array)`, () => {
          return layer.forward(nodeArray, neighborsArray, weightsArray);
        }, 1000);
        printResult(arrayResult);
      } catch (e) {
        console.log(`    Layer forward (Array): Error - ${e.message}`);
      }

      try {
        const float32Result = benchmark(`Layer forward (Float32)`, () => {
          return layer.forward(nodeFloat32, neighborsFloat32, weightsFloat32);
        }, 1000);
        printResult(float32Result);
      } catch (e) {
        console.log(`    Layer forward (Float32): Error - ${e.message}`);
      }
    }
  }
}

// TensorCompress benchmarks
function benchmarkTensorCompress() {
  if (!gnnNative) return;

  console.log('\n📊 TensorCompress Benchmarks');
  console.log('============================');

  const dims = [128, 256, 512, 768, 1024];

  const compressor = new gnnNative.TensorCompress();

  for (const dim of dims) {
    console.log(`\n  Dimension: ${dim}`);

    const embeddingArray = generateRandomVector(dim);
    const embeddingFloat32 = new Float32Array(embeddingArray);

    // Test with Array (triggers conversion)
    try {
      const arrayResult = benchmark(`Compress Array (freq=0.5)`, () => {
        return compressor.compress(embeddingArray, 0.5);
      }, 1000);
      printResult(arrayResult);
    } catch (e) {
      console.log(`    Compress Array: Error - ${e.message}`);
    }

    // Test with Float32Array (zero-copy)
    try {
      const float32Result = benchmark(`Compress Float32 (freq=0.5)`, () => {
        return compressor.compress(embeddingFloat32, 0.5);
      }, 1000);
      printResult(float32Result);
    } catch (e) {
      console.log(`    Compress Float32: Error - ${e.message}`);
    }

    // Decompress benchmark
    try {
      const compressed = compressor.compress(embeddingFloat32, 0.5);
      const decompressResult = benchmark(`Decompress`, () => {
        return compressor.decompress(compressed);
      }, 1000);
      printResult(decompressResult);
    } catch (e) {
      console.log(`    Decompress: Error - ${e.message}`);
    }
  }
}

// Memory allocation benchmarks
function benchmarkMemoryAllocation() {
  console.log('\n📊 Memory Allocation Patterns');
  console.log('=============================');

  const dim = 256;
  const count = 1000;

  // Regular array creation
  printResult(benchmark(`Create ${count} regular arrays (${dim}d)`, () => {
    const arrays = [];
    for (let i = 0; i < count; i++) {
      arrays.push(new Array(dim).fill(0).map(() => Math.random()));
    }
    return arrays;
  }, 100));

  // Float32Array creation
  printResult(benchmark(`Create ${count} Float32Arrays (${dim}d)`, () => {
    const arrays = [];
    for (let i = 0; i < count; i++) {
      const arr = new Float32Array(dim);
      for (let j = 0; j < dim; j++) arr[j] = Math.random();
      arrays.push(arr);
    }
    return arrays;
  }, 100));

  // Pre-allocated buffer
  printResult(benchmark(`Pre-allocated buffer (${count * dim} floats)`, () => {
    const buffer = new Float32Array(count * dim);
    for (let i = 0; i < buffer.length; i++) {
      buffer[i] = Math.random();
    }
    return buffer;
  }, 100));
}

// Main
async function main() {
  console.log('🚀 RuVector GNN Performance Benchmark Suite');
  console.log('============================================\n');

  console.log('System Info:');
  console.log(`  Platform: ${process.platform}`);
  console.log(`  Node.js: ${process.version}`);
  console.log(`  CPU: ${require('os').cpus()[0].model}`);
  console.log(`  Memory: ${Math.round(require('os').totalmem() / 1024 / 1024 / 1024)}GB`);

  benchmarkArrayConversion();
  benchmarkMemoryAllocation();
  benchmarkGnnOperations();
  benchmarkRuvectorLayer();
  benchmarkTensorCompress();
  benchmarkBatchOperations();

  console.log('\n\n📋 Performance Optimization Recommendations');
  console.log('============================================');
  console.log('1. Avoid Array.from() conversion - use typed arrays directly');
  console.log('2. Cache converted arrays when possible');
  console.log('3. Use pre-allocated buffers for batch operations');
  console.log('4. Implement native batch search for multiple queries');
  console.log('5. Consider zero-copy operations with SharedArrayBuffer');
}

main().catch(console.error);
