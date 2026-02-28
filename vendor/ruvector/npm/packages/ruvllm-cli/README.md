# @ruvector/ruvllm-cli

[![npm version](https://img.shields.io/npm/v/@ruvector/ruvllm-cli.svg)](https://www.npmjs.com/package/@ruvector/ruvllm-cli)
[![npm downloads](https://img.shields.io/npm/dt/@ruvector/ruvllm-cli.svg)](https://www.npmjs.com/package/@ruvector/ruvllm-cli)
[![npm downloads/month](https://img.shields.io/npm/dm/@ruvector/ruvllm-cli.svg)](https://www.npmjs.com/package/@ruvector/ruvllm-cli)
[![License](https://img.shields.io/npm/l/@ruvector/ruvllm-cli.svg)](https://github.com/ruvnet/ruvector/blob/main/LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0-blue.svg)](https://www.typescriptlang.org/)

**Command-line interface for local LLM inference and benchmarking** - run AI models on your machine with Metal, CUDA, and CPU acceleration.

## Features

- **Hardware Acceleration** - Metal (macOS), CUDA (NVIDIA), Vulkan, Apple Neural Engine
- **GGUF Support** - Load quantized models (Q4, Q5, Q6, Q8) for efficient inference
- **Interactive Chat** - Terminal-based chat sessions with conversation history
- **Benchmarking** - Measure tokens/second, memory usage, time-to-first-token
- **HTTP Server** - OpenAI-compatible API server for integration
- **Model Management** - Download, list, and manage models from HuggingFace
- **Streaming Output** - Real-time token streaming for responsive UX

## Installation

```bash
# Install globally
npm install -g @ruvector/ruvllm-cli

# Or run directly with npx
npx @ruvector/ruvllm-cli --help
```

For full native performance, install the Rust binary:

```bash
cargo install ruvllm-cli
```

## Quick Start

### Run Inference

```bash
# Basic inference
ruvllm run --model ./llama-7b-q4.gguf --prompt "Explain quantum computing"

# With options
ruvllm run \
  --model ./model.gguf \
  --prompt "Write a haiku about Rust" \
  --temperature 0.8 \
  --max-tokens 100 \
  --backend metal
```

### Interactive Chat

```bash
# Start chat session
ruvllm chat --model ./model.gguf

# With system prompt
ruvllm chat --model ./model.gguf --system "You are a helpful coding assistant"
```

### Benchmark Performance

```bash
# Run benchmark
ruvllm bench --model ./model.gguf --iterations 20

# Compare backends
ruvllm bench --model ./model.gguf --backend metal
ruvllm bench --model ./model.gguf --backend cpu
```

### Start Server

```bash
# OpenAI-compatible API server
ruvllm serve --model ./model.gguf --port 8080

# Then use with any OpenAI client
curl http://localhost:8080/v1/completions \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Hello", "max_tokens": 50}'
```

### Model Management

```bash
# List available models
ruvllm list

# Download from HuggingFace
ruvllm download TheBloke/Llama-2-7B-GGUF

# Download specific quantization
ruvllm download TheBloke/Llama-2-7B-GGUF --quant q4_k_m
```

## CLI Reference

| Command | Description |
|---------|-------------|
| `run` | Run inference on a prompt |
| `chat` | Interactive chat session |
| `bench` | Benchmark model performance |
| `serve` | Start HTTP server |
| `list` | List downloaded models |
| `download` | Download model from HuggingFace |

### Global Options

| Option | Description | Default |
|--------|-------------|---------|
| `--model, -m` | Path to GGUF model file | - |
| `--backend, -b` | Acceleration backend (metal, cuda, cpu) | auto |
| `--threads, -t` | Number of CPU threads | auto |
| `--gpu-layers` | Layers to offload to GPU | all |
| `--context-size` | Context window size | 2048 |
| `--verbose, -v` | Enable verbose logging | false |

### Generation Options

| Option | Description | Default |
|--------|-------------|---------|
| `--temperature` | Sampling temperature (0-2) | 0.7 |
| `--top-p` | Nucleus sampling threshold | 0.9 |
| `--top-k` | Top-k sampling | 40 |
| `--max-tokens` | Maximum tokens to generate | 256 |
| `--repeat-penalty` | Repetition penalty | 1.1 |

## Programmatic Usage

```typescript
import {
  parseArgs,
  formatBenchmarkTable,
  getAvailableBackends,
  ModelConfig,
  BenchmarkResult,
} from '@ruvector/ruvllm-cli';

// Parse CLI arguments
const args = parseArgs(['--model', './model.gguf', '--temperature', '0.8']);
console.log(args); // { model: './model.gguf', temperature: '0.8' }

// Check available backends
const backends = getAvailableBackends();
console.log('Available:', backends); // ['cpu', 'metal'] on macOS

// Format benchmark results
const results: BenchmarkResult[] = [
  {
    model: 'llama-7b',
    backend: 'metal',
    promptTokens: 50,
    generatedTokens: 100,
    promptTime: 120,
    generationTime: 2500,
    promptTPS: 416.7,
    generationTPS: 40.0,
    memoryUsage: 4200,
    peakMemory: 4800,
  },
];

console.log(formatBenchmarkTable(results));
```

## Performance

Benchmarks on Apple M2 Pro with Q4_K_M quantization:

| Model | Prompt TPS | Gen TPS | Memory |
|-------|------------|---------|--------|
| Llama-2-7B | 450 | 42 | 4.2 GB |
| Mistral-7B | 480 | 45 | 4.1 GB |
| Phi-2 | 820 | 85 | 1.8 GB |
| TinyLlama-1.1B | 1200 | 120 | 0.8 GB |

## Configuration

Create `~/.ruvllm/config.json`:

```json
{
  "defaultBackend": "metal",
  "modelsDir": "~/.ruvllm/models",
  "cacheDir": "~/.ruvllm/cache",
  "streaming": true,
  "logLevel": "info"
}
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RUVLLM_MODELS_DIR` | Models directory |
| `RUVLLM_CACHE_DIR` | Cache directory |
| `RUVLLM_BACKEND` | Default backend |
| `RUVLLM_THREADS` | CPU threads |
| `HF_TOKEN` | HuggingFace token for gated models |

## Related Packages

- [@ruvector/ruvllm](https://www.npmjs.com/package/@ruvector/ruvllm) - LLM orchestration library
- [@ruvector/ruvllm-wasm](https://www.npmjs.com/package/@ruvector/ruvllm-wasm) - Browser LLM inference
- [ruvector](https://www.npmjs.com/package/ruvector) - All-in-one vector database

## Documentation

- [RuvLLM Documentation](https://github.com/ruvnet/ruvector/tree/main/crates/ruvllm)
- [CLI Crate](https://github.com/ruvnet/ruvector/tree/main/crates/ruvllm-cli)
- [API Reference](https://docs.rs/ruvllm-cli)

## License

MIT OR Apache-2.0

---

**Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem** - High-performance vector database with self-learning capabilities.
