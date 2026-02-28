# rvf-server

TCP/HTTP streaming server exposing the RuVector Format runtime via a REST API.

## Overview

`rvf-server` wraps `rvf-runtime` in an Axum-based HTTP server for networked vector operations:

- **REST API** -- CRUD endpoints for vectors and collections
- **Streaming** -- chunked transfer for large batch imports/exports
- **CLI** -- configurable via `clap` command-line arguments

## Usage

```bash
cargo run -p rvf-server -- --port 8080
```

## License

MIT OR Apache-2.0
