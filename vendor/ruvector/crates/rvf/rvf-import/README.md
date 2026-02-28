# rvf-import

Data import tools for migrating vectors from JSON, CSV, and NumPy formats into RVF stores.

## What It Does

`rvf-import` provides both a library API and a CLI binary for importing vector data from common formats into `.rvf` files. Supports automatic ID generation, metadata extraction, and batch ingestion.

## Supported Formats

| Format | Extension | Features |
|--------|-----------|----------|
| **JSON** | `.json` | Configurable ID/vector/metadata field names |
| **CSV** | `.csv` | Header-based column mapping, configurable delimiter |
| **NumPy** | `.npy` | Direct binary array loading, auto-dimension detection |

## Library Usage

```rust
use rvf_import::json::{parse_json_file, JsonConfig};

let config = JsonConfig {
    id_field: "id".into(),
    vector_field: "embedding".into(),
    ..Default::default()
};
let records = parse_json_file(Path::new("vectors.json"), &config)?;
```

## CLI Usage

```bash
rvf-import --input data.npy --output vectors.rvf --format npy --dimension 384
rvf-import --input data.csv --output vectors.rvf --format csv --dimension 128
rvf-import --input data.json --output vectors.rvf --format json
```

## Tests

```bash
cargo test -p rvf-import
```

## License

MIT OR Apache-2.0
