# rvf-runtime

RuVector Format runtime providing the RvfStore API, background compaction, and streaming I/O.

## Overview

`rvf-runtime` is the main entry point for applications that want to read and write RVF files:

- **RvfStore** -- high-level API for storing and retrieving vectors
- **Compaction** -- background merge of segments to reclaim space
- **Streaming I/O** -- append-only writes with configurable flush policy

## Usage

```toml
[dependencies]
rvf-runtime = "0.1"
```

## Features

- `std` (default) -- enable `std` I/O support
- `wasm` -- enable WASM-compatible runtime paths

## Lineage Derivation

`RvfStore` supports DNA-style derivation chains where a parent store produces child stores with provenance linkage.

### `derive()` Method

Creates a child store that records this store as its parent. The child gets a new `file_id`, inherits dimensions and options, and records the parent's manifest hash for later verification:

```rust
use rvf_runtime::{RvfStore, options::RvfOptions};
use rvf_types::DerivationType;
use std::path::Path;

let parent = RvfStore::create(Path::new("parent.rvf"), options)?;
let child = parent.derive(
    Path::new("child.rvf"),
    DerivationType::Filter,
    None, // inherit parent options
)?;
assert_eq!(child.lineage_depth(), 1);
```

### FileIdentity Accessors

| Method | Return | Description |
|--------|--------|-------------|
| `file_id()` | `&[u8; 16]` | This file's unique identifier |
| `parent_id()` | `&[u8; 16]` | Parent file's identifier (zeros if root) |
| `lineage_depth()` | `u32` | Derivation depth (0 for root files) |
| `file_identity()` | `&FileIdentity` | Full 68-byte identity struct |

### Extension Aliasing and Domain Profiles

RVF files can use domain-specific extensions that are automatically detected on `create()` and `open()`:

| Extension | Domain Profile | Optimized For |
|-----------|---------------|---------------|
| `.rvf` | Generic | General-purpose vectors |
| `.rvdna` | RVDNA | Genomic sequence embeddings |
| `.rvtext` | RVText | Language model embeddings |
| `.rvgraph` | RVGraph | Graph/network node embeddings |
| `.rvvis` | RVVision | Image/vision model embeddings |

When a child is derived with `derive()`, the child's extension also controls its domain profile. For example, deriving a `.rvdna` child from a `.rvf` parent automatically sets the child's profile to RVDNA.

### FIDI Magic Marker

When `FileIdentity` is present (non-zero `file_id`), the manifest segment includes a 4-byte FIDI magic marker trailer followed by the 68-byte `FileIdentity`. This ensures backward compatibility: old readers that do not recognize the FIDI marker simply stop parsing the manifest payload at the expected end and ignore the trailing bytes.

## Computational Container

`rvf-runtime` provides low-level write-path support for the two computational container segment types defined in [ADR-030](../../../docs/adr/ADR-030-rvf-computational-container.md): KERNEL_SEG (`0x0E`) and EBPF_SEG (`0x0F`).

### Internal Write-Path API

The `SegmentWriter` exposes internal methods for writing computational container segments:

- `write_kernel_seg()` -- writes a KERNEL_SEG containing a 128-byte `KernelHeader`, a compressed kernel image, and an optional kernel command line.
- `write_ebpf_seg()` -- writes an EBPF_SEG containing a 64-byte `EbpfHeader`, eBPF program bytecode (ELF object), and optional BTF data.

These are `pub(crate)` methods used by the segment codec layer. Public `embed_kernel()` / `extract_kernel()` and `embed_ebpf()` / `extract_ebpf()` convenience methods on `RvfStore` are planned for Phase 2 and Phase 3 of the computational container implementation but are not yet available.

### Unknown Segment Preservation

During compaction, `rvf-runtime` preserves segments with unknown or unrecognized types. This means KERNEL_SEG and EBPF_SEG payloads written by newer tooling are retained even when compaction is performed by a runtime version that predates the computational container feature. The compactor copies unknown segments verbatim to the compacted output.

### Example: Writing a Test Stub Kernel Segment

```rust
use std::io::Cursor;

// Build a 128-byte KernelHeader (raw bytes for now; typed struct planned)
let mut kernel_header = [0u8; 128];
// Magic: "RVKN" (0x52564B4E) little-endian
kernel_header[0..4].copy_from_slice(&0x52564B4E_u32.to_le_bytes());
// header_version = 1
kernel_header[4..6].copy_from_slice(&1_u16.to_le_bytes());
// arch = 0x00 (x86_64)
kernel_header[6] = 0x00;
// kernel_type = 0xFE (TestStub)
kernel_header[7] = 0xFE;
// kernel_flags: HAS_QUERY_API (bit 4) | HAS_ADMIN_API (bit 6)
kernel_header[8..12].copy_from_slice(&0x0050_u32.to_le_bytes());
// min_memory_mb = 32
kernel_header[12..16].copy_from_slice(&32_u32.to_le_bytes());

let fake_image = b"test-kernel-stub";
let cmdline = b"console=ttyS0";

// Use the internal SegmentWriter (not public API)
// let (seg_id, offset) = writer.write_kernel_seg(
//     &mut output, &kernel_header, fake_image, Some(cmdline),
// )?;
```

### Planned Public API (Not Yet Implemented)

The following methods are planned for `RvfStore`:

```rust
// Embed a kernel image into the store's .rvf file
// store.embed_kernel(kernel_image, kernel_config)?;

// Extract the kernel image from an existing .rvf file
// let (header, image) = store.extract_kernel()?;

// Embed an eBPF program
// store.embed_ebpf(program_elf, ebpf_config)?;

// Extract the eBPF program
// let (header, elf) = store.extract_ebpf()?;
```

## License

MIT OR Apache-2.0
