# rvf-types

Core type definitions for the RuVector Format (RVF) binary container.

## Overview

`rvf-types` defines the foundational types shared across all RVF crates:

- **Segment headers** -- magic bytes, version, flags, checksums
- **Enums** -- element types (`F32`, `F16`, `U8`, `Binary`), compression modes, distance metrics
- **Flags** -- segment-level feature flags (SIMD hints, encryption, quantization tier)

## Features

- `std` -- enable `std` support (disabled by default for `no_std` compatibility)
- `serde` -- derive `Serialize`/`Deserialize` on all public types

## Usage

```toml
[dependencies]
rvf-types = "0.1"
```

```rust
use rvf_types::{SegmentHeader, ElementType, DistanceMetric};
```

## Lineage / Derivation Types

`rvf-types` defines the types that power DNA-style lineage provenance for RVF files.

### `DerivationType` Enum

Describes how a child file was produced from its parent (`#[repr(u8)]`):

| Variant | Value | Meaning |
|---------|-------|---------|
| `Clone` | 0 | Exact copy of the parent |
| `Filter` | 1 | Subset of parent data |
| `Merge` | 2 | Multiple parents merged |
| `Quantize` | 3 | Re-quantized from parent |
| `Reindex` | 4 | HNSW rebuild or similar |
| `Transform` | 5 | Arbitrary transformation |
| `Snapshot` | 6 | Point-in-time snapshot |
| `UserDefined` | 0xFF | Application-specific derivation |

### `FileIdentity` Struct (68 bytes, `repr(C)`)

Embedded in the Level0Root reserved area at offset `0xF00`. Old readers that ignore the reserved area see zeros and continue working.

| Offset | Size | Field |
|--------|------|-------|
| `0x00` | 16 | `file_id` -- UUID-style unique identifier |
| `0x10` | 16 | `parent_id` -- parent file identifier (zeros for root) |
| `0x20` | 32 | `parent_hash` -- SHAKE-256-256 of parent manifest (zeros for root) |
| `0x40` | 4 | `lineage_depth` -- 0 for root, incremented per derivation |

```rust
use rvf_types::FileIdentity;

let root = FileIdentity::new_root([0x42u8; 16]);
assert!(root.is_root());
assert_eq!(root.lineage_depth, 0);
```

### `LineageRecord` Struct (128 bytes)

A fixed-size record for witness chain entries carrying full derivation metadata:

| Offset | Size | Field |
|--------|------|-------|
| `0x00` | 16 | `file_id` |
| `0x10` | 16 | `parent_id` |
| `0x20` | 32 | `parent_hash` |
| `0x40` | 1 | `derivation_type` |
| `0x41` | 3 | padding |
| `0x44` | 4 | `mutation_count` |
| `0x48` | 8 | `timestamp_ns` |
| `0x50` | 1 | `description_len` |
| `0x51` | 47 | `description` (UTF-8, max 47 bytes) |

### Lineage Witness Type Constants

These extend the witness chain event types for derivation tracking:

| Constant | Value | Purpose |
|----------|-------|---------|
| `WITNESS_DERIVATION` | `0x09` | File derivation event |
| `WITNESS_LINEAGE_MERGE` | `0x0A` | Multi-parent merge |
| `WITNESS_LINEAGE_SNAPSHOT` | `0x0B` | Snapshot event |
| `WITNESS_LINEAGE_TRANSFORM` | `0x0C` | Transform event |
| `WITNESS_LINEAGE_VERIFY` | `0x0D` | Lineage verification |

### `HAS_LINEAGE` Segment Flag

Bit 11 (`0x0800`) of the segment flags bitfield. Set when a file carries DNA-style lineage provenance metadata:

```rust
use rvf_types::SegmentFlags;

let flags = SegmentFlags::empty().with(SegmentFlags::HAS_LINEAGE);
assert!(flags.contains(SegmentFlags::HAS_LINEAGE));
assert_eq!(flags.bits(), 0x0800);
```

### Lineage Error Codes (Category `0x06`)

| Code | Name | Description |
|------|------|-------------|
| `0x0600` | `ParentNotFound` | Referenced parent file not found |
| `0x0601` | `ParentHashMismatch` | Parent hash does not match recorded `parent_hash` |
| `0x0602` | `LineageBroken` | Lineage chain has a missing link |
| `0x0603` | `LineageCyclic` | Lineage chain contains a cycle |

## Computational Container Types

`rvf-types` defines the segment types and header structures for the RVF computational container model, which allows `.rvf` files to carry executable compute alongside vector data.

### Segment Types

Two segment type discriminants support the computational container:

| Variant | Value | Description |
|---------|-------|-------------|
| `SegmentType::Kernel` | `0x0E` | Embedded kernel / unikernel image for self-booting |
| `SegmentType::Ebpf` | `0x0F` | Embedded eBPF program for kernel fast path |

These are defined in `segment_type.rs` and round-trip through `TryFrom<u8>`:

```rust
use rvf_types::SegmentType;

assert_eq!(SegmentType::Kernel as u8, 0x0E);
assert_eq!(SegmentType::Ebpf as u8, 0x0F);
assert_eq!(SegmentType::try_from(0x0E), Ok(SegmentType::Kernel));
```

### `KernelHeader` (128 bytes, `repr(C)`)

Describes an embedded unikernel or micro-Linux image within a KERNEL_SEG payload. Follows the standard 64-byte `SegmentHeader`.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| `0x00` | 4 | `kernel_magic` | Magic: `0x52564B4E` ("RVKN") |
| `0x04` | 2 | `header_version` | KernelHeader format version (currently 1) |
| `0x06` | 1 | `arch` | Target architecture (`KernelArch` enum) |
| `0x07` | 1 | `kernel_type` | Kernel type (`KernelType` enum) |
| `0x08` | 4 | `kernel_flags` | Bitfield flags (`KernelFlags`) |
| `0x0C` | 4 | `min_memory_mb` | Minimum RAM required (MiB) |
| `0x10` | 8 | `entry_point` | Virtual address of kernel entry point |
| `0x18` | 8 | `image_size` | Uncompressed kernel image size (bytes) |
| `0x20` | 8 | `compressed_size` | Compressed kernel image size (bytes) |
| `0x28` | 1 | `compression` | Compression algorithm (same as SegmentHeader) |
| `0x29` | 1 | `api_transport` | API transport (`ApiTransport` enum) |
| `0x2A` | 2 | `api_port` | Default API port (network byte order) |
| `0x2C` | 4 | `api_version` | Supported RVF query API version |
| `0x30` | 32 | `image_hash` | SHAKE-256-256 of uncompressed kernel image |
| `0x50` | 16 | `build_id` | Unique build identifier (UUID v7) |
| `0x60` | 8 | `build_timestamp` | Build time (nanosecond UNIX timestamp) |
| `0x68` | 4 | `vcpu_count` | Recommended vCPU count (0 = single) |
| `0x6C` | 4 | `reserved_0` | Reserved (must be zero) |
| `0x70` | 8 | `cmdline_offset` | Offset to kernel command line within payload |
| `0x78` | 4 | `cmdline_length` | Length of kernel command line (bytes) |
| `0x7C` | 4 | `reserved_1` | Reserved (must be zero) |

### `EbpfHeader` (64 bytes, `repr(C)`)

Describes an embedded eBPF program within an EBPF_SEG payload.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| `0x00` | 4 | `ebpf_magic` | Magic: `0x52564250` ("RVBP") |
| `0x04` | 2 | `header_version` | EbpfHeader format version (currently 1) |
| `0x06` | 1 | `program_type` | eBPF program type (`EbpfProgramType` enum) |
| `0x07` | 1 | `attach_type` | eBPF attach point (`EbpfAttachType` enum) |
| `0x08` | 4 | `program_flags` | Bitfield flags |
| `0x0C` | 2 | `insn_count` | Number of BPF instructions (max 65535) |
| `0x0E` | 2 | `max_dimension` | Maximum vector dimension this program handles |
| `0x10` | 8 | `program_size` | ELF object size (bytes) |
| `0x18` | 4 | `map_count` | Number of BPF maps defined |
| `0x1C` | 4 | `btf_size` | BTF (BPF Type Format) section size |
| `0x20` | 32 | `program_hash` | SHAKE-256-256 of the ELF object |

### Enums

#### `KernelArch` (`#[repr(u8)]`)

| Value | Name | Description |
|-------|------|-------------|
| `0x00` | `X86_64` | AMD64 / Intel 64 |
| `0x01` | `Aarch64` | ARM 64-bit (ARMv8-A and later) |
| `0x02` | `Riscv64` | RISC-V 64-bit (RV64GC) |
| `0xFE` | `Universal` | Architecture-independent (e.g., interpreted) |
| `0xFF` | `Unknown` | Reserved / unspecified |

#### `KernelType` (`#[repr(u8)]`)

| Value | Name | Description |
|-------|------|-------------|
| `0x00` | `Hermit` | Hermit OS unikernel (Rust-native) |
| `0x01` | `MicroLinux` | Minimal Linux kernel (bzImage compatible) |
| `0x02` | `Asterinas` | Asterinas framekernel (Linux ABI compatible) |
| `0x03` | `WasiPreview2` | WASI Preview 2 component |
| `0x04` | `Custom` | Custom kernel (requires external VMM knowledge) |
| `0xFE` | `TestStub` | Test stub for CI (boots, reports health, exits) |
| `0xFF` | `Reserved` | Reserved |

#### `ApiTransport` (`#[repr(u8)]`)

| Value | Name | Description |
|-------|------|-------------|
| `0x00` | `TcpHttp` | HTTP/1.1 over TCP (default) |
| `0x01` | `TcpGrpc` | gRPC over TCP (HTTP/2) |
| `0x02` | `Vsock` | VirtIO socket (Firecracker host-to-guest) |
| `0x03` | `SharedMem` | Shared memory region (same-host co-location) |
| `0xFF` | `None` | No network API (batch mode only) |

#### `EbpfProgramType` (`#[repr(u8)]`)

| Value | Name | Description |
|-------|------|-------------|
| `0x00` | `XdpDistance` | XDP program for distance computation on packets |
| `0x01` | `TcFilter` | TC classifier for query routing |
| `0x02` | `SocketFilter` | Socket filter for query preprocessing |
| `0x03` | `Tracepoint` | Tracepoint for performance monitoring |
| `0x04` | `Kprobe` | Kprobe for dynamic instrumentation |
| `0x05` | `CgroupSkb` | Cgroup socket buffer filter |
| `0xFF` | `Custom` | Custom program type |

#### `EbpfAttachType` (`#[repr(u8)]`)

| Value | Name | Description |
|-------|------|-------------|
| `0x00` | `XdpIngress` | XDP hook on NIC ingress |
| `0x01` | `TcIngress` | TC ingress qdisc |
| `0x02` | `TcEgress` | TC egress qdisc |
| `0x03` | `SocketFilter` | Socket filter attachment |
| `0x04` | `CgroupIngress` | Cgroup ingress |
| `0x05` | `CgroupEgress` | Cgroup egress |
| `0xFF` | `None` | No automatic attachment |

### `KernelFlags` Constants (u32 bitfield)

| Bit | Name | Description |
|-----|------|-------------|
| 0 | `REQUIRES_TEE` | Kernel must run inside a TEE enclave |
| 1 | `REQUIRES_KVM` | Kernel requires KVM (hardware virtualization) |
| 2 | `REQUIRES_UEFI` | Kernel requires UEFI boot |
| 3 | `HAS_NETWORKING` | Kernel includes network stack |
| 4 | `HAS_QUERY_API` | Kernel exposes RVF query API on `api_port` |
| 5 | `HAS_INGEST_API` | Kernel exposes RVF ingest API |
| 6 | `HAS_ADMIN_API` | Kernel exposes health/metrics API |
| 7 | `ATTESTATION_READY` | Kernel can generate TEE attestation quotes |
| 8 | `SIGNED` | Kernel image is signed (SignatureFooter follows) |
| 9 | `MEASURED` | Kernel measurement stored in WITNESS_SEG |
| 10 | `COMPRESSED` | Image is compressed (per compression field) |
| 11 | `RELOCATABLE` | Kernel is position-independent |
| 12 | `HAS_VIRTIO_NET` | Kernel includes VirtIO network driver |
| 13 | `HAS_VIRTIO_BLK` | Kernel includes VirtIO block driver |
| 14 | `HAS_VSOCK` | Kernel includes VSOCK for host communication |
| 15-31 | reserved | Reserved (must be zero) |

### Three-Tier Execution Model

RVF supports a three-tier execution model where a single `.rvf` file can carry compute at multiple levels:

| Tier | Segment | Typical Size | Target Environment | Boot Time |
|------|---------|-------------|--------------------|-----------|
| **1: WASM** | WASM_SEG (existing) | 5.5 KB | Browser, edge, IoT | <1 ms |
| **2: eBPF** | EBPF_SEG (`0x0F`) | 10-50 KB | Linux kernel fast path (XDP, TC) | <20 ms |
| **3: Unikernel** | KERNEL_SEG (`0x0E`) | 200 KB - 2 MB | TEE enclaves, Firecracker, bare metal | <125 ms |

Files without KERNEL_SEG or EBPF_SEG continue to work unchanged. Readers that do not recognize these segment types skip them per the RVF forward-compatibility rule. See [ADR-030](../../../docs/adr/ADR-030-rvf-computational-container.md) for the full specification.

## License

MIT OR Apache-2.0
