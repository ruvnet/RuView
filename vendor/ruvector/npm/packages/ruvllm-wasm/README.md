# @ruvector/ruvllm-wasm

[![npm version](https://img.shields.io/npm/v/@ruvector/ruvllm-wasm.svg)](https://www.npmjs.com/package/@ruvector/ruvllm-wasm)
[![npm downloads](https://img.shields.io/npm/dt/@ruvector/ruvllm-wasm.svg)](https://www.npmjs.com/package/@ruvector/ruvllm-wasm)
[![npm downloads/month](https://img.shields.io/npm/dm/@ruvector/ruvllm-wasm.svg)](https://www.npmjs.com/package/@ruvector/ruvllm-wasm)
[![License](https://img.shields.io/npm/l/@ruvector/ruvllm-wasm.svg)](https://github.com/ruvnet/ruvector/blob/main/LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0-blue.svg)](https://www.typescriptlang.org/)

**Run large language models directly in the browser** using WebAssembly with optional WebGPU acceleration for faster inference.

## Features

- **Browser-Native** - No server required, runs entirely client-side
- **WebGPU Acceleration** - 10-50x faster inference with GPU support
- **GGUF Models** - Load quantized models for efficient browser inference
- **Streaming** - Real-time token streaming for responsive UX
- **IndexedDB Caching** - Cache models locally for instant reload
- **Privacy-First** - All processing happens on-device
- **SIMD Support** - Optimized WASM with SIMD instructions
- **Multi-Threading** - Parallel inference with SharedArrayBuffer

## Installation

```bash
npm install @ruvector/ruvllm-wasm
```

## Quick Start

```typescript
import { RuvLLMWasm, checkWebGPU } from '@ruvector/ruvllm-wasm';

// Check browser capabilities
const webgpu = await checkWebGPU();
console.log('WebGPU:', webgpu); // 'available' | 'unavailable' | 'not_supported'

// Create instance with WebGPU (if available)
const llm = await RuvLLMWasm.create({
  useWebGPU: true,
  memoryLimit: 4096, // 4GB max
});

// Load a model (with progress tracking)
await llm.loadModel('https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf', {
  onProgress: (loaded, total) => {
    console.log(`Loading: ${Math.round(loaded / total * 100)}%`);
  }
});

// Generate text
const result = await llm.generate('What is the capital of France?', {
  maxTokens: 100,
  temperature: 0.7,
});

console.log(result.text);
console.log(`${result.stats.tokensPerSecond.toFixed(1)} tokens/sec`);
```

## Streaming Tokens

```typescript
// Stream tokens as they're generated
await llm.generate('Tell me a story about a robot', {
  maxTokens: 200,
  stream: true,
}, (token, done) => {
  process.stdout.write(token);
  if (done) console.log('\n--- Done ---');
});
```

## Chat Interface

```typescript
import { ChatMessage } from '@ruvector/ruvllm-wasm';

const messages: ChatMessage[] = [
  { role: 'system', content: 'You are a helpful assistant.' },
  { role: 'user', content: 'What is 2 + 2?' },
];

const response = await llm.chat(messages, {
  maxTokens: 100,
  temperature: 0.5,
});

console.log(response.text); // "2 + 2 equals 4."
```

## React Hook Example

```tsx
import { useState, useEffect } from 'react';
import { RuvLLMWasm, LoadingStatus } from '@ruvector/ruvllm-wasm';

function useLLM(modelUrl: string) {
  const [llm, setLLM] = useState<RuvLLMWasm | null>(null);
  const [status, setStatus] = useState<LoadingStatus>('idle');
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    let instance: RuvLLMWasm;

    async function init() {
      instance = await RuvLLMWasm.create({ useWebGPU: true });
      setStatus('downloading');

      await instance.loadModel(modelUrl, {
        onProgress: (loaded, total) => setProgress(loaded / total),
      });

      setStatus('ready');
      setLLM(instance);
    }

    init();
    return () => instance?.unload();
  }, [modelUrl]);

  return { llm, status, progress };
}

// Usage
function ChatApp() {
  const { llm, status, progress } = useLLM('https://example.com/model.gguf');
  const [response, setResponse] = useState('');

  if (status !== 'ready') {
    return <div>Loading: {Math.round(progress * 100)}%</div>;
  }

  const generate = async () => {
    const result = await llm!.generate('Hello!', { maxTokens: 50 });
    setResponse(result.text);
  };

  return (
    <div>
      <button onClick={generate}>Generate</button>
      <p>{response}</p>
    </div>
  );
}
```

## Browser Requirements

| Feature | Required | Benefit |
|---------|----------|---------|
| WebAssembly | Yes | Core execution |
| WebGPU | No (recommended) | 10-50x faster |
| SharedArrayBuffer | No | Multi-threading |
| SIMD | No | 2-4x faster math |

### Check Capabilities

```typescript
import { getCapabilities } from '@ruvector/ruvllm-wasm';

const caps = await getCapabilities();
console.log(caps);
// {
//   webgpu: 'available',
//   sharedArrayBuffer: true,
//   simd: true,
//   crossOriginIsolated: true
// }
```

### Enable SharedArrayBuffer

Add these headers to your server:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

## API Reference

### `RuvLLMWasm.create(options?)`

Create a new instance.

```typescript
const llm = await RuvLLMWasm.create({
  useWebGPU: true,      // Enable WebGPU acceleration
  threads: 4,           // CPU threads (requires SharedArrayBuffer)
  memoryLimit: 4096,    // Max memory in MB
});
```

### `loadModel(source, options?)`

Load a GGUF model.

```typescript
await llm.loadModel(url, {
  onProgress: (loaded, total) => { /* ... */ }
});
```

### `generate(prompt, config?, onToken?)`

Generate text completion.

```typescript
const result = await llm.generate('Hello', {
  maxTokens: 100,
  temperature: 0.7,
  topP: 0.9,
  topK: 40,
  repetitionPenalty: 1.1,
  stopSequences: ['\n\n'],
  stream: true,
}, (token, done) => { /* ... */ });
```

### `chat(messages, config?, onToken?)`

Chat completion with message history.

```typescript
const result = await llm.chat([
  { role: 'system', content: 'You are helpful.' },
  { role: 'user', content: 'Hi!' },
], { maxTokens: 100 });
```

### `unload()`

Free memory and unload model.

```typescript
llm.unload();
```

## Recommended Models

Small models suitable for browser inference:

| Model | Size | Use Case |
|-------|------|----------|
| TinyLlama-1.1B-Q4 | ~700 MB | General chat |
| Phi-2-Q4 | ~1.6 GB | Code, reasoning |
| Qwen2-0.5B-Q4 | ~400 MB | Fast responses |
| StableLM-Zephyr-3B-Q4 | ~2 GB | Quality chat |

## Performance Tips

1. **Use WebGPU** - Check support and enable for 10-50x speedup
2. **Smaller models** - Q4_K_M quantization balances quality/size
3. **Cache models** - IndexedDB caching avoids re-downloads
4. **Limit context** - Smaller context = faster inference
5. **Stream tokens** - Better UX with progressive output

## Related Packages

- [@ruvector/ruvllm](https://www.npmjs.com/package/@ruvector/ruvllm) - Node.js LLM library
- [@ruvector/ruvllm-cli](https://www.npmjs.com/package/@ruvector/ruvllm-cli) - CLI tool
- [ruvector](https://www.npmjs.com/package/ruvector) - Vector database

## Documentation

- [WASM Crate](https://github.com/ruvnet/ruvector/tree/main/crates/ruvllm-wasm)
- [API Reference](https://docs.rs/ruvllm-wasm)
- [Examples](https://github.com/ruvnet/ruvector/tree/main/examples/ruvLLM)

## License

MIT OR Apache-2.0

---

**Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem** - High-performance vector database with self-learning capabilities.
