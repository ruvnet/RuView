# rvf-wire

Zero-copy wire format reader and writer for RuVector Format (RVF) segments.

## Overview

`rvf-wire` handles serialization and deserialization of RVF binary segments:

- **Writer** -- append segments with automatic CRC32c and XXH3 checksums
- **Reader** -- stream-parse segments with validation and integrity checks
- **Zero-copy** -- borrows directly from memory-mapped buffers where possible

## Usage

```toml
[dependencies]
rvf-wire = "0.1"
```

```rust
use rvf_wire::{SegmentWriter, SegmentReader};
```

## Features

- `std` (default) -- enable `std` I/O support

## License

MIT OR Apache-2.0
