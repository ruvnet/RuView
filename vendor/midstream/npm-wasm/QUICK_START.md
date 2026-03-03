# Midstream WASM Quick Start Guide

## Installation

```bash
npm install @midstream/wasm
```

Or use from CDN:
```html
<script type="module">
  import init, { TemporalCompare } from 'https://unpkg.com/@midstream/wasm';
  await init();
  // Use the module
</script>
```

## Basic Usage

### 1. Temporal Comparison (DTW, LCS, Edit Distance)

```javascript
import init, { TemporalCompare } from '@midstream/wasm';

await init();

const tc = new TemporalCompare();

// Dynamic Time Warping
const seq1 = new Float64Array([1, 2, 3, 4, 5]);
const seq2 = new Float64Array([1, 2, 3, 4, 5]);
const distance = tc.dtw(seq1, seq2);
console.log('DTW distance:', distance); // 0.0

// Longest Common Subsequence
const lcsSeq1 = new Int32Array([1, 2, 3, 4, 5]);
const lcsSeq2 = new Int32Array([1, 3, 5]);
const length = tc.lcs(lcsSeq1, lcsSeq2);
console.log('LCS length:', length); // 3

// Edit Distance
const editDist = tc.edit_distance('kitten', 'sitting');
console.log('Edit distance:', editDist); // 3

// Comprehensive Analysis
const metrics = tc.analyze(seq1, seq2);
console.log('Metrics:', {
  dtw: metrics.dtw_distance,
  lcs: metrics.lcs_length,
  edit: metrics.edit_distance,
  similarity: metrics.similarity_score
});
```

### 2. Nanosecond Scheduler (Browser Only)

```javascript
import init, { NanoScheduler } from '@midstream/wasm';

await init();

const scheduler = new NanoScheduler();

// Schedule a task
const taskId = scheduler.schedule(() => {
  console.log('Task executed!');
}, 1000000000); // 1 second in nanoseconds

// Start scheduler loop
function loop() {
  const executed = scheduler.tick();
  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);

// Cancel a task
scheduler.cancel(taskId);

// Get pending count
console.log('Pending tasks:', scheduler.pending_count);
```

### 3. Meta-Learning (Strange Loop)

```javascript
import init, { StrangeLoop } from '@midstream/wasm';

await init();

const loop = new StrangeLoop(0.1); // Learning rate

// Observe patterns
loop.observe('fast-path', 0.8);
loop.observe('slow-path', 0.3);
loop.observe('cached', 0.95);

// Get confidence
const confidence = loop.get_confidence('cached');
console.log('Confidence:', confidence);

// Find best pattern
const best = loop.best_pattern();
console.log('Best pattern:', best.pattern_id, best.confidence);

// Meta-cognition reflection
const reflection = loop.reflect();
console.log('All patterns:', reflection);
```

### 4. QUIC Multistream

```javascript
import init, { QuicMultistream } from '@midstream/wasm';

await init();

const quic = new QuicMultistream();

// Open stream with priority
const streamId = quic.open_stream(200);

// Send data
const data = new Uint8Array([1, 2, 3, 4, 5]);
const sent = quic.send(streamId, data);

// Receive data
const received = quic.receive(streamId, 1024);

// Get statistics
const stats = quic.get_stats(streamId);
console.log('Stats:', stats);
/*
{
  stream_id: 0,
  priority: 200,
  bytes_sent: 5,
  bytes_received: 1024
}
*/

// Close stream
quic.close_stream(streamId);
```

## Performance Benchmarking

```javascript
import init, { benchmark_dtw } from '@midstream/wasm';

await init();

// Benchmark DTW algorithm
const avgTime = benchmark_dtw(100, 100);
console.log(`Average time: ${avgTime.toFixed(3)}ms`);
console.log(`Throughput: ${(1000 / avgTime).toFixed(0)} ops/sec`);
```

## Module Information

```javascript
import init, { version } from '@midstream/wasm';

await init();

console.log('Version:', version()); // "1.0.0"
```

## Advanced Usage

### Custom Window Size for DTW

```javascript
const tc = new TemporalCompare(200); // Custom window size
const distance = tc.dtw(largeSeq1, largeSeq2);
```

### Repeating Tasks

```javascript
const scheduler = new NanoScheduler();

const taskId = scheduler.schedule_repeating(() => {
  console.log('Repeating task');
}, 500000000); // Every 0.5 seconds

// Cancel when done
setTimeout(() => scheduler.cancel(taskId), 5000);
```

### Learning Over Time

```javascript
const loop = new StrangeLoop(0.1);

// Simulate learning progression
for (let i = 0; i < 100; i++) {
  const performance = 0.5 + Math.random() * 0.5;
  loop.observe('my-pattern', performance);
}

console.log('Iterations:', loop.iteration_count);
console.log('Final confidence:', loop.get_confidence('my-pattern'));
```

### Multiple Streams

```javascript
const quic = new QuicMultistream();

// Open multiple streams with different priorities
const high = quic.open_stream(255);
const medium = quic.open_stream(128);
const low = quic.open_stream(50);

console.log('Total streams:', quic.stream_count);

// Close all
quic.close_stream(high);
quic.close_stream(medium);
quic.close_stream(low);
```

## Memory Management

All classes have a `free()` method to manually release memory:

```javascript
const tc = new TemporalCompare();
// Use tc...
tc.free(); // Release WASM memory
```

However, JavaScript's garbage collector will automatically clean up when objects go out of scope.

## TypeScript Support

Full TypeScript definitions are included:

```typescript
import init, {
  TemporalCompare,
  NanoScheduler,
  StrangeLoop,
  QuicMultistream,
  TemporalMetrics,
  MetaPattern
} from '@midstream/wasm';

await init();

const tc: TemporalCompare = new TemporalCompare();
const metrics: TemporalMetrics = tc.analyze(seq1, seq2);
```

## Browser Compatibility

**Minimum Requirements**:
- WebAssembly support
- ES6 modules
- Typed Arrays (Float64Array, Int32Array, Uint8Array)

**Optional Features**:
- `window.performance` (for NanoScheduler)
- Modern browser (Chrome 57+, Firefox 52+, Safari 11+, Edge 16+)

## Node.js Usage

Most features work in Node.js 18+, except:
- ❌ NanoScheduler (requires browser `window` object)
- ✅ TemporalCompare (full support)
- ✅ StrangeLoop (full support)
- ✅ QuicMultistream (full support)

```javascript
// Node.js example
import { readFile } from 'fs/promises';
import { TemporalCompare } from '@midstream/wasm';

const wasmBuffer = await readFile('./node_modules/@midstream/wasm/midstream_wasm_bg.wasm');
const wasmModule = await WebAssembly.compile(wasmBuffer);
await init(wasmModule);

const tc = new TemporalCompare();
// Use tc...
```

## Examples

See the `examples/` directory for:
- Interactive demo (`examples/demo.html`)
- Browser tests (`tests/browser_test.html`)
- Performance benchmarks

## Performance Tips

1. **Reuse instances**: Create TemporalCompare once, use many times
2. **Typed Arrays**: Always use Float64Array/Int32Array for best performance
3. **Batch operations**: Process multiple sequences in one call
4. **Memory cleanup**: Call `free()` on large objects when done

## Troubleshooting

### "Failed to load WASM module"
- Ensure WASM file is served with correct MIME type (`application/wasm`)
- Check that the WASM file is accessible

### "no global window" error
- NanoScheduler only works in browsers
- Use other modules in Node.js

### Performance issues
- Check input size (DTW is O(n²))
- Use appropriate window size
- Consider Web Workers for heavy computation

## License

MIT

## Links

- [GitHub Repository](https://github.com/midstream/midstream)
- [Documentation](https://github.com/midstream/midstream/blob/main/README.md)
- [Issue Tracker](https://github.com/midstream/midstream/issues)
