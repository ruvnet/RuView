# RuvLLM CLI

Command-line interface for RuvLLM inference, optimized for Apple Silicon.

## Installation

```bash
# From crates.io
cargo install ruvllm-cli

# From source (with Metal acceleration)
cargo install --path . --features metal
```

## Commands

### Download Models

Download models from HuggingFace Hub:

```bash
# Download Qwen with Q4K quantization (default)
ruvllm download qwen

# Download with specific quantization
ruvllm download qwen --quantization q8
ruvllm download mistral --quantization f16

# Force re-download
ruvllm download phi --force

# Download specific revision
ruvllm download llama --revision main
```

#### Model Aliases

| Alias | Model ID |
|-------|----------|
| `qwen` | `Qwen/Qwen2.5-7B-Instruct` |
| `mistral` | `mistralai/Mistral-7B-Instruct-v0.3` |
| `phi` | `microsoft/Phi-3-medium-4k-instruct` |
| `llama` | `meta-llama/Meta-Llama-3.1-8B-Instruct` |

#### Quantization Options

| Option | Description | Memory Savings |
|--------|-------------|----------------|
| `q4k` | 4-bit quantization (default) | ~75% |
| `q8` | 8-bit quantization | ~50% |
| `f16` | Half precision | ~50% |
| `none` | Full precision | 0% |

### List Models

```bash
# List all available models
ruvllm list

# List only downloaded models
ruvllm list --downloaded

# Detailed listing with sizes
ruvllm list --long
```

### Model Information

```bash
# Show model details
ruvllm info qwen

# Output includes:
# - Model architecture
# - Parameter count
# - Download status
# - Disk usage
# - Supported features
```

### Interactive Chat

```bash
# Start chat with default settings
ruvllm chat qwen

# With custom system prompt
ruvllm chat qwen --system "You are a helpful coding assistant."

# Adjust generation parameters
ruvllm chat qwen --temperature 0.5 --max-tokens 1024

# Use specific quantization
ruvllm chat qwen --quantization q8
```

#### Chat Commands

During chat, use these commands:

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/clear` | Clear conversation history |
| `/system <prompt>` | Change system prompt |
| `/temp <value>` | Change temperature |
| `/quit` or `/exit` | Exit chat |

### Start Server

OpenAI-compatible inference server:

```bash
# Start with defaults
ruvllm serve qwen

# Custom host and port
ruvllm serve qwen --host 0.0.0.0 --port 8080

# Configure concurrency
ruvllm serve qwen --max-concurrent 8 --max-context 8192
```

#### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/chat/completions` | POST | Chat completions |
| `/v1/completions` | POST | Text completions |
| `/v1/models` | GET | List models |
| `/health` | GET | Health check |

#### Example Request

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ],
    "max_tokens": 256
  }'
```

### Run Benchmarks

```bash
# Basic benchmark
ruvllm benchmark qwen

# Configure benchmark
ruvllm benchmark qwen \
  --warmup 5 \
  --iterations 20 \
  --prompt-length 256 \
  --gen-length 128

# Output formats
ruvllm benchmark qwen --format json
ruvllm benchmark qwen --format csv
```

#### Benchmark Metrics

- **Prefill Latency**: Time to process input prompt
- **Decode Throughput**: Tokens per second during generation
- **Time to First Token (TTFT)**: Latency before first output token
- **Memory Usage**: Peak GPU/RAM consumption

## Global Options

```bash
# Enable verbose logging
ruvllm --verbose <command>

# Disable colored output
ruvllm --no-color <command>

# Custom cache directory
ruvllm --cache-dir /path/to/cache <command>

# Or via environment variable
export RUVLLM_CACHE_DIR=/path/to/cache
```

## Configuration

### Cache Directory

Models are cached in:

- **macOS**: `~/Library/Caches/ruvllm`
- **Linux**: `~/.cache/ruvllm`
- **Windows**: `%LOCALAPPDATA%\ruvllm`

Override with `--cache-dir` or `RUVLLM_CACHE_DIR`.

### Logging

Set log level with `RUST_LOG`:

```bash
RUST_LOG=debug ruvllm chat qwen
RUST_LOG=ruvllm=trace ruvllm serve qwen
```

## Examples

### Basic Workflow

```bash
# 1. Download a model
ruvllm download qwen

# 2. Verify it's downloaded
ruvllm list --downloaded

# 3. Start chatting
ruvllm chat qwen
```

### Server Deployment

```bash
# Download model first
ruvllm download qwen --quantization q4k

# Start server with production settings
ruvllm serve qwen \
  --host 0.0.0.0 \
  --port 8080 \
  --max-concurrent 16 \
  --max-context 4096 \
  --quantization q4k
```

### Performance Testing

```bash
# Run comprehensive benchmarks
ruvllm benchmark qwen \
  --warmup 10 \
  --iterations 50 \
  --prompt-length 512 \
  --gen-length 256 \
  --format json > benchmark_results.json
```

## Troubleshooting

### Out of Memory

```bash
# Use smaller quantization
ruvllm chat qwen --quantization q4k

# Or reduce context length
ruvllm serve qwen --max-context 2048
```

### Slow Download

```bash
# Resume interrupted download
ruvllm download qwen

# Force fresh download
ruvllm download qwen --force
```

### Metal Issues (macOS)

Ensure Metal is available:

```bash
# Check Metal device
system_profiler SPDisplaysDataType | grep Metal

# Try with CPU fallback
RUVLLM_NO_METAL=1 ruvllm chat qwen
```

## Feature Flags

Build with specific features:

```bash
# Metal acceleration (macOS)
cargo install ruvllm-cli --features metal

# CUDA acceleration (NVIDIA)
cargo install ruvllm-cli --features cuda

# Both (if available)
cargo install ruvllm-cli --features "metal,cuda"
```

## License

Apache-2.0 / MIT dual license.
