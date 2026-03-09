# ruv-neural-cli

CLI tool for brain topology analysis, simulation, and visualization.

## Overview

`ruv-neural-cli` is the command-line binary (`ruv-neural`) that ties together
the entire rUv Neural crate ecosystem. It provides subcommands for simulating
neural sensor data, analyzing brain connectivity graphs, computing minimum cuts,
running the full processing pipeline with an optional ASCII dashboard, and
exporting to multiple visualization formats.

## Installation

```bash
# Build from source
cargo install --path .

# Or run directly
cargo run -p ruv-neural-cli -- <command>
```

## Commands

### `simulate` -- Generate synthetic neural data

```bash
ruv-neural simulate --channels 64 --duration 10 --sample-rate 1000 --output data.json
```

| Flag             | Default | Description                  |
|------------------|---------|------------------------------|
| `-c, --channels` | 64      | Number of sensor channels    |
| `-d, --duration` | 10.0    | Duration in seconds          |
| `-s, --sample-rate` | 1000.0 | Sample rate in Hz         |
| `-o, --output`   | (none)  | Output file path (JSON)      |

### `analyze` -- Analyze a brain connectivity graph

```bash
ruv-neural analyze --input graph.json --ascii --csv metrics.csv
```

| Flag           | Default | Description                    |
|----------------|---------|--------------------------------|
| `-i, --input`  | (required) | Input graph file (JSON)    |
| `--ascii`      | false   | Show ASCII visualization       |
| `--csv`        | (none)  | Export metrics to CSV file     |

### `mincut` -- Compute minimum cut

```bash
ruv-neural mincut --input graph.json --k 4
```

| Flag           | Default | Description                    |
|----------------|---------|--------------------------------|
| `-i, --input`  | (required) | Input graph file (JSON)    |
| `-k`           | (none)  | Multi-way cut with k partitions|

### `pipeline` -- Full end-to-end pipeline

```bash
ruv-neural pipeline --channels 32 --duration 5 --dashboard
```

Runs: simulate -> preprocess -> build graph -> mincut -> embed -> decode.

| Flag             | Default | Description                    |
|------------------|---------|--------------------------------|
| `-c, --channels` | 32      | Number of sensor channels      |
| `-d, --duration` | 5.0     | Duration in seconds            |
| `--dashboard`    | false   | Show real-time ASCII dashboard |

### `export` -- Export to visualization format

```bash
ruv-neural export --input graph.json --format dot --output graph.dot
```

| Flag             | Default | Description                           |
|------------------|---------|---------------------------------------|
| `-i, --input`    | (required) | Input graph file (JSON)           |
| `-f, --format`   | d3      | Output format: d3, dot, gexf, csv, rvf |
| `-o, --output`   | (required) | Output file path                  |

### `info` -- Show system information

```bash
ruv-neural info
```

Displays crate versions, available features, and system capabilities.

## Global Options

| Flag             | Description                        |
|------------------|------------------------------------|
| `-v`             | Increase verbosity (up to `-vvv`)  |
| `--version`      | Print version                      |
| `--help`         | Print help                         |

## Integration

Depends on all workspace crates: `ruv-neural-core`, `ruv-neural-sensor`,
`ruv-neural-signal`, `ruv-neural-graph`, `ruv-neural-mincut`, `ruv-neural-embed`,
`ruv-neural-memory`, `ruv-neural-decoder`, and `ruv-neural-viz`. Uses `clap`
for argument parsing and `tokio` for async runtime.

## License

MIT OR Apache-2.0
