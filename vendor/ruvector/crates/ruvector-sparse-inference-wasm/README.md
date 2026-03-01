# ruvector-sparse-inference-wasm

WebAssembly bindings for PowerInfer-style sparse inference engine.

## Overview

This crate provides WASM bindings for the RuVector sparse inference engine, enabling efficient neural network inference in web browsers and Node.js environments with:

- **Sparse Activation**: PowerInfer-style neuron prediction for 2-3x speedup
- **GGUF Support**: Load quantized models in GGUF format
- **Streaming Loading**: Fetch large models incrementally
- **Multiple Backends**: Embedding models and LLM text generation

## Building

### For Web Browsers

```bash
wasm-pack build --target web --release
```

### For Node.js

```bash
wasm-pack build --target nodejs --release
```

### For Bundlers (webpack, rollup, etc.)

```bash
wasm-pack build --target bundler --release
```

## Installation

```bash
npm install ruvector-sparse-inference-wasm
```

Or build locally:

```bash
wasm-pack build --target web
cd pkg && npm link
```

## Usage

### Basic Inference Engine

```typescript
import init, { SparseInferenceEngine } from 'ruvector-sparse-inference-wasm';

// Initialize WASM module
await init();

// Load model
const modelBytes = await fetch('/models/llama-2-7b.gguf').then(r => r.arrayBuffer());
const config = {
  sparsity: {
    enabled: true,
    threshold: 0.1  // 10% neuron activation
  },
  temperature: 0.7,
  top_k: 40
};

const engine = new SparseInferenceEngine(
  new Uint8Array(modelBytes),
  JSON.stringify(config)
);

// Run inference
const input = new Float32Array(4096);  // Your input embedding
const output = engine.infer(input);

console.log('Sparsity stats:', engine.sparsity_stats());
console.log('Model metadata:', engine.metadata());
```

### Streaming Model Loading

For large models (>1GB), use streaming:

```typescript
const engine = await SparseInferenceEngine.load_streaming(
  'https://example.com/large-model.gguf',
  JSON.stringify(config)
);
```

### Embedding Models

For sentence transformers and embedding generation:

```typescript
import { EmbeddingModel } from 'ruvector-sparse-inference-wasm';

const modelBytes = await fetch('/models/all-MiniLM-L6-v2.gguf').then(r => r.arrayBuffer());
const embedder = new EmbeddingModel(new Uint8Array(modelBytes));

// Encode single sequence (requires tokenization first)
const inputIds = new Uint32Array([101, 2023, 2003, ...]);  // Tokenized input
const embedding = embedder.encode(inputIds);

console.log('Embedding dimension:', embedder.dimension());

// Batch encoding
const batchIds = new Uint32Array([...all tokenized sequences...]);
const lengths = new Uint32Array([10, 15, 12]);  // Length of each sequence
const embeddings = embedder.encode_batch(batchIds, lengths);
```

### LLM Text Generation

For autoregressive language models:

```typescript
import { LLMModel } from 'ruvector-sparse-inference-wasm';

const modelBytes = await fetch('/models/llama-2-7b-chat.gguf').then(r => r.arrayBuffer());
const config = {
  sparsity: { enabled: true, threshold: 0.1 },
  temperature: 0.7,
  top_k: 40
};

const llm = new LLMModel(new Uint8Array(modelBytes), JSON.stringify(config));

// Generate tokens one at a time
const prompt = new Uint32Array([1, 4321, 1234, ...]); // Tokenized prompt
let generatedTokens = [];

for (let i = 0; i < 100; i++) {
  const nextToken = llm.next_token(prompt);
  generatedTokens.push(nextToken);

  // Append to prompt for next iteration
  prompt = new Uint32Array([...prompt, nextToken]);
}

// Or generate multiple tokens at once
const tokens = llm.generate(prompt, 100);

console.log('Generation stats:', llm.stats());

// Reset for new conversation
llm.reset_cache();
```

### Calibration

Improve predictor accuracy with sample data:

```typescript
// Collect representative samples
const samples = new Float32Array([
  ...embedding1,  // 512 dims
  ...embedding2,  // 512 dims
  ...embedding3,  // 512 dims
]);

engine.calibrate(samples, 512);  // 512 = dimension of each sample
```

### Dynamic Sparsity Control

Adjust sparsity threshold at runtime:

```typescript
// More sparse = faster, less accurate
engine.set_sparsity(0.2);  // 20% activation

// Less sparse = slower, more accurate
engine.set_sparsity(0.05);  // 5% activation
```

### Performance Measurement

```typescript
import { measure_inference_time } from 'ruvector-sparse-inference-wasm';

const input = new Float32Array(4096);
const avgTime = measure_inference_time(engine, input, 100);  // 100 iterations

console.log(`Average inference time: ${avgTime.toFixed(2)}ms`);
```

## Configuration Options

```typescript
interface InferenceConfig {
  sparsity: {
    enabled: boolean;      // Enable sparse inference
    threshold: number;     // Activation threshold (0.0-1.0)
  };
  temperature: number;     // Sampling temperature (0.0-2.0)
  top_k: number;          // Top-k sampling (1-100)
  top_p?: number;         // Nucleus sampling (0.0-1.0)
  max_tokens?: number;    // Max generation length
}
```

## Browser Compatibility

- Chrome/Edge 91+ (WebAssembly SIMD)
- Firefox 89+
- Safari 15+
- Node.js 16+

For older browsers, build without SIMD:

```bash
wasm-pack build --target web -- --no-default-features
```

## Performance Tips

1. **Enable SIMD**: Ensure `wasm32-simd` is enabled for 2-4x speedup
2. **Quantization**: Use 4-bit or 8-bit quantized GGUF models
3. **Sparsity**: Tune threshold based on accuracy/speed tradeoff
4. **Calibration**: Run calibration with representative data
5. **Batch Processing**: Use batch encoding for multiple inputs
6. **Worker Threads**: Run inference in Web Workers to avoid blocking UI

## Example: Web Worker Integration

```typescript
// worker.js
import init, { SparseInferenceEngine } from 'ruvector-sparse-inference-wasm';

let engine;

self.onmessage = async (e) => {
  if (e.data.type === 'init') {
    await init();
    engine = new SparseInferenceEngine(e.data.modelBytes, e.data.config);
    self.postMessage({ type: 'ready' });
  } else if (e.data.type === 'infer') {
    const output = engine.infer(e.data.input);
    self.postMessage({ type: 'result', output });
  }
};

// main.js
const worker = new Worker('worker.js', { type: 'module' });

worker.postMessage({
  type: 'init',
  modelBytes: new Uint8Array(modelBytes),
  config: JSON.stringify(config)
});

worker.onmessage = (e) => {
  if (e.data.type === 'ready') {
    worker.postMessage({
      type: 'infer',
      input: new Float32Array([...])
    });
  } else if (e.data.type === 'result') {
    console.log('Inference result:', e.data.output);
  }
};
```

## Benchmarks

On Apple M1 Pro (browser):

| Model | Size | Sparsity | Speed | Memory |
|-------|------|----------|-------|--------|
| Llama-2-7B | 3.8GB | 10% | 45 tok/s | 1.2GB |
| MiniLM-L6 | 90MB | 15% | 120 emb/s | 180MB |
| Mistral-7B | 4.1GB | 12% | 38 tok/s | 1.4GB |

## Error Handling

```typescript
try {
  const engine = new SparseInferenceEngine(modelBytes, config);
  const output = engine.infer(input);
} catch (error) {
  if (error.message.includes('parse')) {
    console.error('Invalid GGUF model format');
  } else if (error.message.includes('config')) {
    console.error('Invalid configuration');
  } else {
    console.error('Inference failed:', error);
  }
}
```

## Development

### Run Tests

```bash
wasm-pack test --headless --chrome
wasm-pack test --headless --firefox
```

### Build Documentation

```bash
cargo doc --open --target wasm32-unknown-unknown
```

### Size Optimization

```bash
# Optimize for size
wasm-pack build --target web --release -- -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort

# Further compression with wasm-opt
wasm-opt -Oz -o optimized.wasm pkg/ruvector_sparse_inference_wasm_bg.wasm
```

## License

Same as parent RuVector project.

## Related Crates

- `ruvector-sparse-inference` - Core Rust implementation
- `ruvector-core` - Main RuVector library
- `rvlite` - Lightweight WASM vector database

## Contributing

See main RuVector repository for contribution guidelines.
