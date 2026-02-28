# RuVector Documentation

Additional documentation and usage guides.

## Contents

| File | Description |
|------|-------------|
| `graph-cli-usage.md` | Command-line interface for graph operations |
| `graph_wasm_usage.html` | Interactive WASM graph demo |

## Graph CLI

The graph CLI provides command-line access to RuVector's graph features:

```bash
ruvector-graph --help
ruvector-graph query "MATCH (n) RETURN n LIMIT 10"
ruvector-graph import data.json
ruvector-graph export output.json
```

See [graph-cli-usage.md](graph-cli-usage.md) for full documentation.

## WASM Demo

Open `graph_wasm_usage.html` in a browser to see an interactive demonstration of RuVector's WebAssembly graph capabilities.

## Additional Resources

- [Main Examples README](../README.md)
- [Rust Examples](../rust/README.md)
- [Node.js Examples](../nodejs/README.md)
- [React + WASM](../wasm-react/README.md)
