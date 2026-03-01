# rvf-manifest

Two-level manifest system for tracking RVF segments and coordinating compaction.

## Overview

`rvf-manifest` manages the metadata layer that tracks which segments are active, tombstoned, or pending compaction:

- **Level-0 manifest** -- per-file segment directory with CRC32c integrity
- **Level-1 manifest** -- cross-file global index for multi-file stores
- **Compaction coordination** -- tracks merge candidates and generation numbers

## Usage

```toml
[dependencies]
rvf-manifest = "0.1"
```

## Features

- `std` (default) -- enable `std` I/O support

## FileIdentity Storage

The `FileIdentity` struct (68 bytes) is stored at offset `0xF00` within the Level0Root reserved area (252 bytes starting at the end of the signature region). This placement is backward compatible: old readers that ignore the reserved area see zeros and continue working normally.

| Offset | Size | Field |
|--------|------|-------|
| `0xF00` | 16 | `file_id` |
| `0xF10` | 16 | `parent_id` |
| `0xF20` | 32 | `parent_hash` |
| `0xF40` | 4 | `lineage_depth` |

The `read_level0()` and `write_level0()` functions in this crate transparently read and write the `FileIdentity` at these offsets. The CRC32C checksum at offset `0xFFC` covers the entire 4092-byte region including the FileIdentity bytes, ensuring integrity.

## License

MIT OR Apache-2.0
