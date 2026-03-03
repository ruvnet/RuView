# @midstream/wasm

WebAssembly bindings for Midstream temporal comparison, nanosecond scheduling, meta-learning, and QUIC multistream functionality.

## Features

- **Temporal Comparison**: Dynamic Time Warping (DTW), Longest Common Subsequence (LCS), and Edit Distance algorithms
- **Nanosecond Scheduler**: High-precision task scheduling with browser-based timing
- **Strange Loop**: Meta-learning and pattern recognition with self-improving algorithms
- **QUIC Multistream**: WebTransport-compatible multiplexed streaming (simulated)

## Installation

```bash
npm install @midstream/wasm
```

## Quick Start

### Browser

```html
<!DOCTYPE html>
<html>
<head>
  <script type="module">
    import { init, TemporalCompare, NanoScheduler, StrangeLoop } from '@midstream/wasm';

    async function main() {
      // Initialize WASM module
      await init();

      // Temporal comparison
      const temporal = new TemporalCompare();
      const seq1 = [1, 2, 3, 4, 5];
      const seq2 = [1, 3, 4, 5];

      const metrics = temporal.analyze(seq1, seq2);
      console.log('Similarity:', metrics.similarityScore);

      // Nanosecond scheduler
      const scheduler = new NanoScheduler();
      scheduler.start();

      scheduler.schedule(() => {
        console.log('Task executed!');
      }, 1000000000); // 1 second in nanoseconds

      // Meta-learning
      const loop = new StrangeLoop(0.1);
      loop.observe('pattern-a', 0.8);
      loop.observe('pattern-b', 0.6);

      const best = loop.bestPattern();
      console.log('Best pattern:', best.patternId);
    }

    main();
  </script>
</head>
<body>
  <h1>Midstream WASM Demo</h1>
</body>
</html>
```

### Node.js

```javascript
import { init, TemporalCompare, NanoScheduler, StrangeLoop, QuicMultistream } from '@midstream/wasm';

async function main() {
  // Initialize WASM module
  await init();

  // Temporal comparison example
  const temporal = new TemporalCompare(100);

  const timeSeries1 = Array.from({ length: 100 }, (_, i) => Math.sin(i / 10));
  const timeSeries2 = Array.from({ length: 100 }, (_, i) => Math.cos(i / 10));

  const dtwDistance = temporal.dtw(timeSeries1, timeSeries2);
  console.log('DTW Distance:', dtwDistance);

  const metrics = temporal.analyze(timeSeries1, timeSeries2);
  console.log('Comprehensive Analysis:', metrics);

  // Nanosecond scheduler example
  const scheduler = new NanoScheduler();
  scheduler.start();

  let counter = 0;
  const taskId = scheduler.scheduleRepeating(() => {
    counter++;
    console.log('Tick:', counter);

    if (counter >= 10) {
      scheduler.cancel(taskId);
      scheduler.stop();
    }
  }, 100000000); // 100ms in nanoseconds

  // Meta-learning example
  const loop = new StrangeLoop(0.15);

  // Simulate learning from observations
  for (let i = 0; i < 100; i++) {
    const performance = Math.random();
    loop.observe(`pattern-${i % 5}`, performance);
  }

  console.log('Iteration count:', loop.iterationCount);
  console.log('Pattern count:', loop.patternCount);
  console.log('Best pattern:', loop.bestPattern());

  // QUIC multistream example
  const quic = new QuicMultistream();

  const streamId = quic.openStream(255); // High priority
  const data = new Uint8Array([1, 2, 3, 4, 5]);

  const bytesSent = quic.send(streamId, data);
  console.log('Bytes sent:', bytesSent);

  const stats = quic.getStats(streamId);
  console.log('Stream stats:', stats);

  quic.closeStream(streamId);
}

main();
```

## API Reference

### Initialization

#### `init(wasmPath?: string): Promise<void>`

Initialize the WASM module. Must be called before using any other API.

```javascript
await init();
```

### TemporalCompare

Temporal sequence comparison algorithms.

#### Constructor

```javascript
const temporal = new TemporalCompare(windowSize?: number);
```

#### Methods

- `dtw(seq1: number[], seq2: number[]): number` - Dynamic Time Warping distance
- `lcs(seq1: number[], seq2: number[]): number` - Longest Common Subsequence length
- `editDistance(s1: string, s2: string): number` - Levenshtein edit distance
- `analyze(seq1: number[], seq2: number[]): TemporalMetrics` - Comprehensive analysis

### NanoScheduler

High-precision task scheduler with nanosecond timing.

#### Constructor

```javascript
const scheduler = new NanoScheduler();
```

#### Methods

- `schedule(callback: () => void, delayNs: number): number` - Schedule one-time task
- `scheduleRepeating(callback: () => void, intervalNs: number): number` - Schedule repeating task
- `cancel(taskId: number): boolean` - Cancel a task
- `nowNs(): number` - Get current time in nanoseconds
- `start(): void` - Start processing tasks
- `stop(): void` - Stop the scheduler

#### Properties

- `pendingCount: number` - Number of pending tasks

### StrangeLoop

Meta-learning and pattern recognition with self-improvement.

#### Constructor

```javascript
const loop = new StrangeLoop(learningRate?: number);
```

#### Methods

- `observe(patternId: string, performance: number): void` - Learn from observation
- `getConfidence(patternId: string): number | null` - Get pattern confidence
- `bestPattern(): MetaPattern | null` - Get best learned pattern
- `reflect(): Record<string, MetaPattern>` - Get all patterns (meta-cognition)

#### Properties

- `iterationCount: number` - Total learning iterations
- `patternCount: number` - Number of learned patterns

### QuicMultistream

WebTransport-compatible multiplexed streaming.

#### Constructor

```javascript
const quic = new QuicMultistream();
```

#### Methods

- `openStream(priority?: number): number` - Open new stream
- `closeStream(streamId: number): boolean` - Close stream
- `send(streamId: number, data: Uint8Array): number` - Send data
- `receive(streamId: number, size: number): Uint8Array` - Receive data
- `getStats(streamId: number): StreamStats | null` - Get stream statistics

#### Properties

- `streamCount: number` - Number of active streams

### Utilities

- `version(): string` - Get WASM module version
- `benchmarkDtw(size?: number, iterations?: number): number` - Benchmark DTW performance

## Performance

The WASM implementation provides significant performance improvements:

- **DTW**: ~100x faster than pure JavaScript
- **LCS**: ~50x faster than pure JavaScript
- **Scheduler**: Microsecond precision using browser Performance API
- **Binary size**: ~80KB (gzipped)

## Browser Compatibility

- Chrome 87+
- Firefox 89+
- Safari 15+
- Edge 88+

Requires WebAssembly support.

## Examples

See the `examples/` directory for complete demonstrations:

- `demo.html` - Interactive browser demo
- `scheduler-demo.html` - Nanosecond scheduler visualization
- `meta-learning-demo.html` - Real-time meta-learning
- `performance-test.html` - Performance benchmarks

## Building from Source

```bash
# Install dependencies
npm install

# Build WASM for all targets
npm run build

# Development mode with hot reload
npm run dev

# Run tests
npm test

# Clean build artifacts
npm run clean
```

## License

MIT

## Contributing

Contributions welcome! Please see the main Midstream repository for guidelines.

## Links

- [GitHub Repository](https://github.com/midstream/midstream)
- [Documentation](https://midstream.dev/docs)
- [Examples](https://midstream.dev/examples)
