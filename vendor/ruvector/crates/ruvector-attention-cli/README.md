# RuVector Attention CLI

A high-performance command-line interface for working with attention mechanisms.

## Features

- **Multiple Attention Types**: Scaled dot-product, multi-head, hyperbolic, flash, linear, and MoE
- **Compute**: Process attention on input data with various configurations
- **Benchmark**: Performance testing across different dimensions and attention types
- **Convert**: Transform data between JSON, binary, MessagePack, CSV formats
- **Serve**: HTTP server with REST API for attention computation
- **REPL**: Interactive shell for exploratory analysis

## Installation

```bash
cargo install --path .
```

## Usage

### Compute Attention

```bash
# Scaled dot-product attention
ruvector-attention compute -i input.json -o output.json -a scaled_dot

# Multi-head attention with 16 heads
ruvector-attention compute -i input.json -a multi_head --num-heads 16

# Hyperbolic attention with custom curvature
ruvector-attention compute -i input.json -a hyperbolic --curvature 2.0

# Flash attention (memory-efficient)
ruvector-attention compute -i input.json -a flash

# Mixture of Experts attention
ruvector-attention compute -i input.json -a moe --num-experts 8 --top-k 2
```

### Run Benchmarks

```bash
# Benchmark all attention types
ruvector-attention benchmark

# Benchmark specific types
ruvector-attention benchmark -a scaled_dot,multi_head,flash

# Custom dimensions
ruvector-attention benchmark -d 256,512,1024 -i 1000

# Output to CSV
ruvector-attention benchmark -o results.csv -f csv
```

### Convert Data

```bash
# JSON to MessagePack
ruvector-attention convert -i data.json -o data.msgpack --to msgpack

# Binary to JSON (pretty-printed)
ruvector-attention convert -i data.bin -o data.json --to json --pretty

# Auto-detect input format
ruvector-attention convert -i input.dat -o output.json --to json
```

### Start HTTP Server

```bash
# Default (localhost:8080)
ruvector-attention serve

# Custom host and port
ruvector-attention serve -H 0.0.0.0 -p 3000

# With CORS enabled
ruvector-attention serve --cors
```

### Interactive REPL

```bash
# Start REPL
ruvector-attention repl

# Commands within REPL:
attention> help
attention> load data.json
attention> type multi_head
attention> compute
attention> config
attention> quit
```

## API Endpoints

When running the server, the following endpoints are available:

- `GET /health` - Health check
- `POST /attention/scaled_dot` - Scaled dot-product attention
- `POST /attention/multi_head` - Multi-head attention
- `POST /attention/hyperbolic` - Hyperbolic attention
- `POST /attention/flash` - Flash attention
- `POST /attention/linear` - Linear attention
- `POST /attention/moe` - Mixture of Experts attention
- `POST /batch` - Batch computation

### Example Request

```bash
curl -X POST http://localhost:8080/attention/scaled_dot \
  -H "Content-Type: application/json" \
  -d '{
    "query": [[0.1, 0.2, 0.3]],
    "keys": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]],
    "values": [[0.7, 0.8, 0.9], [1.0, 1.1, 1.2]]
  }'
```

## Configuration

Create a `ruvector-attention.toml` file:

```toml
[attention]
default_dim = 512
default_heads = 8
default_type = "scaled_dot"

[server]
host = "0.0.0.0"
port = 8080
max_batch_size = 32

[output]
format = "json"
pretty = true

[benchmark]
iterations = 100
dimensions = [128, 256, 512, 1024]
```

## Input Format

Input files should contain:

```json
{
  "query": [[...], [...], ...],
  "keys": [[...], [...], ...],
  "values": [[...], [...], ...],
  "dim": 512
}
```

## Performance

Benchmark results on typical hardware:

| Attention Type | 512-dim | 1024-dim | 2048-dim |
|---------------|---------|----------|----------|
| Scaled Dot    | 0.5ms   | 1.2ms    | 4.8ms    |
| Multi-Head    | 1.2ms   | 3.5ms    | 14.2ms   |
| Flash         | 0.3ms   | 0.8ms    | 3.1ms    |
| Linear        | 0.4ms   | 1.0ms    | 3.9ms    |

## License

MIT OR Apache-2.0
