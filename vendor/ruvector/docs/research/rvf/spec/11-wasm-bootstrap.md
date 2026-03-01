# RVF WASM Self-Bootstrapping Specification

## 1. Motivation

Traditional file formats require an external runtime to interpret their contents.
A JPEG needs an image decoder. A SQLite database needs the SQLite library. An RVF
file needs a vector search engine.

What if the file carried its own runtime?

By embedding a tiny WASM interpreter inside the RVF file itself, we eliminate the
last external dependency. The host only needs **raw execution capability** — the
ability to run bytes as instructions. RVF becomes **self-bootstrapping**: a single
file that contains both its data and the complete machinery to process that data.

This is the transition from "needs a compatible runtime" to **"runs anywhere
compute exists."**

## 2. Architecture

### The Bootstrap Stack

```
Layer 3:  RVF Data Segments          (VEC_SEG, INDEX_SEG, MANIFEST_SEG, ...)
            ^
            | processes
            |
Layer 2:  WASM Microkernel           (WASM_SEG, role=Microkernel, ~5.5 KB)
            ^                         14 exports: query, ingest, distance, top-K
            | executes
            |
Layer 1:  WASM Interpreter           (WASM_SEG, role=Interpreter, ~50 KB)
            ^                         Minimal stack machine that runs WASM bytecode
            | loads
            |
Layer 0:  Raw Bytes                  (The .rvf file on any storage medium)
```

Each layer depends only on the one below it. The host reads Layer 0 (raw bytes),
finds the interpreter at Layer 1, uses it to execute the microkernel at Layer 2,
which then processes the data at Layer 3.

### Segment Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│                         bootable.rvf                                 │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────┐ │
│  │  WASM_SEG    │  │  WASM_SEG    │  │  VEC_SEG     │  │ INDEX   │ │
│  │  0x10        │  │  0x10        │  │  0x01        │  │ _SEG    │ │
│  │              │  │              │  │              │  │ 0x02    │ │
│  │ role=Interp  │  │ role=uKernel │  │ 10M vectors  │  │ HNSW    │ │
│  │ ~50 KB       │  │ ~5.5 KB      │  │ 384-dim fp16 │  │ L0+L1   │ │
│  │ priority=0   │  │ priority=1   │  │              │  │         │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └─────────┘ │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │ QUANT_SEG    │  │ WITNESS_SEG  │  │ MANIFEST_SEG │  ← tail      │
│  │ codebooks    │  │ audit trail  │  │ source of    │               │
│  │              │  │              │  │ truth        │               │
│  └──────────────┘  └──────────────┘  └──────────────┘               │
└──────────────────────────────────────────────────────────────────────┘
```

## 3. WASM_SEG Wire Format

### Segment Type

```
Value:  0x10
Name:   WASM_SEG
```

Uses the standard 64-byte RVF segment header (`SegmentHeader`), followed by
a 64-byte `WasmHeader`, followed by the WASM bytecode.

### WasmHeader (64 bytes)

```
Offset  Size  Type    Field               Description
------  ----  ----    -----               -----------
0x00    4     u32     wasm_magic           0x5256574D ("RVWM" big-endian)
0x04    2     u16     header_version       Currently 1
0x06    1     u8      role                 Bootstrap role (see WasmRole enum)
0x07    1     u8      target               Target platform (see WasmTarget enum)
0x08    2     u16     required_features    WASM feature bitfield
0x0A    2     u16     export_count         Number of WASM exports
0x0C    4     u32     bytecode_size        Uncompressed bytecode size (bytes)
0x10    4     u32     compressed_size      Compressed size (0 = no compression)
0x14    1     u8      compression          0=none, 1=LZ4, 2=ZSTD
0x15    1     u8      min_memory_pages     Minimum linear memory (64 KB each)
0x16    1     u8      max_memory_pages     Maximum linear memory (0 = no limit)
0x17    1     u8      table_count          Number of WASM tables
0x18    32    hash256 bytecode_hash        SHAKE-256-256 of uncompressed bytecode
0x38    1     u8      bootstrap_priority   Lower = tried first in chain
0x39    1     u8      interpreter_type     Interpreter variant (if role=Interpreter)
0x3A    6     u8[6]   reserved             Must be zero
```

### WasmRole Enum

```
Value  Name            Description
-----  ----            -----------
0x00   Microkernel     RVF query engine (5.5 KB Cognitum tile runtime)
0x01   Interpreter     Minimal WASM interpreter for self-bootstrapping
0x02   Combined        Interpreter + microkernel linked together
0x03   Extension       Domain-specific module (custom distance, decoder)
0x04   ControlPlane    Store management (create, export, segment parsing)
```

### WasmTarget Enum

```
Value  Name         Description
-----  ----         -----------
0x00   Wasm32       Generic wasm32 (any compliant runtime)
0x01   WasiP1       WASI Preview 1 (requires WASI syscalls)
0x02   WasiP2       WASI Preview 2 (component model)
0x03   Browser      Browser-optimized (expects Web APIs)
0x04   BareTile     Bare-metal Cognitum tile (hub-tile protocol only)
```

### Required Features Bitfield

```
Bit  Mask    Feature
---  ----    -------
0    0x0001  SIMD (v128 operations)
1    0x0002  Bulk memory operations
2    0x0004  Multi-value returns
3    0x0008  Reference types
4    0x0010  Threads (shared memory)
5    0x0020  Tail call optimization
6    0x0040  GC (garbage collection)
7    0x0080  Exception handling
```

### Interpreter Type (when role=Interpreter)

```
Value  Name              Description
-----  ----              -----------
0x00   StackMachine      Generic stack-based interpreter
0x01   Wasm3Compatible   wasm3-style (register machine)
0x02   WamrCompatible    WAMR-style (AOT + interpreter)
0x03   WasmiCompatible   wasmi-style (pure stack machine)
```

## 4. Bootstrap Resolution Protocol

### Discovery

1. Scan all segments for `seg_type == 0x10` (WASM_SEG)
2. Parse the 64-byte WasmHeader from each
3. Validate `wasm_magic == 0x5256574D`
4. Sort by `bootstrap_priority` ascending

### Resolution

```
IF any WASM_SEG has role=Combined:
    → SelfContained bootstrap (single module does everything)

ELIF WASM_SEG with role=Interpreter AND role=Microkernel both exist:
    → TwoStage bootstrap (interpreter runs microkernel)

ELIF only WASM_SEG with role=Microkernel exists:
    → HostRequired (needs external WASM runtime)

ELSE:
    → No WASM bootstrap available
```

### Execution Sequence (Two-Stage)

```
Host                    Interpreter              Microkernel           Data
 |                         |                        |                   |
 |-- read WASM_SEG[0] --->|                        |                   |
 |   (interpreter bytes)   |                        |                   |
 |                         |                        |                   |
 |-- instantiate -------->|                        |                   |
 |   (load into memory)    |                        |                   |
 |                         |                        |                   |
 |-- feed WASM_SEG[1] --->|-- instantiate -------->|                   |
 |   (microkernel bytes)   |   (via interpreter)    |                   |
 |                         |                        |                   |
 |-- LOAD_QUERY --------->|------- forward ------->|                   |
 |                         |                        |-- read VEC_SEG -->|
 |                         |                        |<- vector block ---|
 |                         |                        |                   |
 |                         |                        |  rvf_distances()  |
 |                         |                        |  rvf_topk_merge() |
 |                         |                        |                   |
 |<-- TOPK_RESULT --------|<------ return ---------|                   |
```

## 5. Size Budget

### Microkernel (role=Microkernel)

Already specified in `microkernel/wasm-runtime.md`:

```
Total:  ~5,500 bytes (< 8 KB code budget)
Exports: 14 (query path + quantization + HNSW + verification)
Memory:  8 KB data + 64 KB SIMD scratch
```

### Interpreter (role=Interpreter)

Target: minimal WASM bytecode interpreter sufficient to run the microkernel.

```
Component                    Estimated Size
---------                    --------------
WASM binary parser           4 KB
  (magic, section parsing)
Type section decoder         1 KB
  (function types)
Import/Export resolution     2 KB
Code section interpreter     12 KB
  (control flow, locals)
Stack machine engine         8 KB
  (operand stack, call stack)
Memory management            3 KB
  (linear memory, grow)
i32/i64 integer ops          4 KB
  (add, sub, mul, div, rem, shifts)
f32/f64 float ops            6 KB
  (add, sub, mul, div, sqrt, conversions)
v128 SIMD ops (optional)     8 KB
  (only if WASM_FEAT_SIMD required)
Table + call_indirect        2 KB
                             ----------
Total (no SIMD):             ~42 KB
Total (with SIMD):           ~50 KB
```

### Combined (role=Combined)

Interpreter linked with microkernel in a single module:

```
Total: ~48-56 KB (interpreter + microkernel, with overlap eliminated)
```

### Self-Bootstrapping Overhead

For a 10M vector file (~7.3 GB at 384-dim fp16):
- Bootstrap overhead: ~56 KB / ~7.3 GB = **0.0008%**
- The file is 99.9992% data, 0.0008% self-sufficient runtime

For a 1000-vector file (~750 KB):
- Bootstrap overhead: ~56 KB / ~750 KB = **7.5%**
- Still practical for edge/IoT deployments

## 6. Execution Tiers (Extended)

The original three-tier model from ADR-030 is extended:

| Tier | Segment | Size | Boot | Self-Bootstrap? |
|------|---------|------|------|-----------------|
| 0: Embedded WASM Interpreter | WASM_SEG (role=Interpreter) | ~50 KB | <5 ms | **Yes** — file carries its own runtime |
| 1: WASM Microkernel | WASM_SEG (role=Microkernel) | 5.5 KB | <1 ms | No — needs host or Tier 0 |
| 2: eBPF | EBPF_SEG | 10-50 KB | <20 ms | No — needs Linux kernel |
| 3: Unikernel | KERNEL_SEG | 200 KB-2 MB | <125 ms | No — needs VMM (Firecracker) |

**Key insight**: Tier 0 makes all other tiers optional. An RVF file with
Tier 0 embedded runs on *any* host that can execute bytes — bare metal,
browser, microcontroller, FPGA with a soft CPU, or even another WASM runtime.

## 7. "Runs Anywhere Compute Exists"

### What This Means

A self-bootstrapping RVF file requires exactly **one capability** from its host:

> The ability to read bytes from storage and execute them as instructions.

That's it. No operating system. No file system. No network stack. No runtime
library. No package manager. No container engine.

### Where It Runs

| Host | How It Works |
|------|-------------|
| **x86 server** | Native WASM runtime (Wasmtime/WAMR) runs microkernel directly |
| **ARM edge device** | Same — native WASM runtime |
| **Browser tab** | `WebAssembly.instantiate()` on the microkernel bytes |
| **Microcontroller** | Embedded interpreter runs microkernel in 64 KB scratch |
| **FPGA soft CPU** | Interpreter mapped to BRAM, microkernel in flash |
| **Another WASM runtime** | Interpreter-in-WASM runs microkernel-in-WASM (turtles) |
| **Bare metal** | Bootloader extracts interpreter, interpreter runs microkernel |
| **TEE enclave** | Enclave loads interpreter, verified via WITNESS_SEG attestation |

### The Bootstrapping Invariant

For any host `H` with execution capability `E`:

```
∀ H, E:  can_execute(H, E) ∧ can_read_bytes(H)
         → can_process_rvf(H, self_bootstrapping_rvf_file)
```

The file is a **fixed point** of the execution relation: it contains everything
needed to process itself.

## 8. Security Considerations

### Interpreter Verification

The embedded interpreter's bytecode is hashed with SHAKE-256-256 and stored
in the WasmHeader (`bytecode_hash`). A WITNESS_SEG can chain the interpreter
hash to a trusted build, providing:

- **Provenance**: Who built this interpreter?
- **Integrity**: Has the interpreter been modified?
- **Attestation**: Can a TEE verify the interpreter before execution?

### Sandbox Guarantees

The WASM sandbox model applies at every layer:
- The interpreter cannot access host memory beyond its linear memory
- The microkernel cannot access interpreter memory
- Each layer communicates only through defined exports/imports
- A trapped module cannot corrupt other modules

### Bootstrap Attack Surface

| Attack | Mitigation |
|--------|-----------|
| Malicious interpreter | Verify `bytecode_hash` against known-good hash in WITNESS_SEG |
| Modified microkernel | Interpreter verifies microkernel hash before instantiation |
| Data corruption | Segment-level CRC32C/SHAKE-256 hashes (Law 2) |
| Code injection | WASM validates all code at load time (type checking) |
| Resource exhaustion | `max_memory_pages` cap, epoch-based interruption |

## 9. API

### Rust (rvf-runtime)

```rust
// Embed a WASM module
store.embed_wasm(
    role: WasmRole::Microkernel as u8,
    target: WasmTarget::Wasm32 as u8,
    required_features: WASM_FEAT_SIMD,
    wasm_bytecode: &microkernel_bytes,
    export_count: 14,
    bootstrap_priority: 1,
    interpreter_type: 0,
)?;

// Make self-bootstrapping
store.embed_wasm(
    role: WasmRole::Interpreter as u8,
    target: WasmTarget::Wasm32 as u8,
    required_features: 0,
    wasm_bytecode: &interpreter_bytes,
    export_count: 3,
    bootstrap_priority: 0,
    interpreter_type: 0x03, // wasmi-compatible
)?;

// Check if file is self-bootstrapping
assert!(store.is_self_bootstrapping());

// Extract all WASM modules (ordered by priority)
let modules = store.extract_wasm_all()?;
```

### WASM (rvf-wasm bootstrap module)

```rust
use rvf_wasm::bootstrap::{resolve_bootstrap_chain, get_bytecode, BootstrapChain};

let chain = resolve_bootstrap_chain(&rvf_bytes);

match chain {
    BootstrapChain::SelfContained { combined } => {
        let bytecode = get_bytecode(&rvf_bytes, &combined).unwrap();
        // Instantiate and run
    }
    BootstrapChain::TwoStage { interpreter, microkernel } => {
        let interp_code = get_bytecode(&rvf_bytes, &interpreter).unwrap();
        let kernel_code = get_bytecode(&rvf_bytes, &microkernel).unwrap();
        // Load interpreter, then use it to run microkernel
    }
    _ => { /* use host runtime */ }
}
```

## 10. Relationship to Existing Segments

| Segment | Relationship to WASM_SEG |
|---------|-------------------------|
| KERNEL_SEG (0x0E) | Alternative execution tier — KERNEL_SEG boots a full unikernel, WASM_SEG runs a lightweight microkernel. Both make the file self-executing but at different capability levels. |
| EBPF_SEG (0x0F) | Complementary — eBPF accelerates hot-path queries on Linux hosts while WASM provides universal portability. |
| WITNESS_SEG (0x0A) | Verification — WITNESS_SEG chains can attest the interpreter and microkernel hashes, providing a trust anchor for the bootstrap chain. |
| CRYPTO_SEG (0x0C) | Signing — CRYPTO_SEG key material can sign WASM_SEG contents for tamper detection. |
| MANIFEST_SEG (0x05) | Discovery — the tail manifest references all WASM_SEGs with their roles and priorities. |

## 11. Implementation Status

| Component | Crate | Status |
|-----------|-------|--------|
| `SegmentType::Wasm` (0x10) | `rvf-types` | Implemented |
| `WasmHeader` (64-byte header) | `rvf-types` | Implemented |
| `WasmRole`, `WasmTarget` enums | `rvf-types` | Implemented |
| `write_wasm_seg` | `rvf-runtime` | Implemented |
| `embed_wasm` / `extract_wasm` | `rvf-runtime` | Implemented |
| `extract_wasm_all` (priority-sorted) | `rvf-runtime` | Implemented |
| `is_self_bootstrapping` | `rvf-runtime` | Implemented |
| `resolve_bootstrap_chain` | `rvf-wasm` | Implemented |
| `get_bytecode` (zero-copy extraction) | `rvf-wasm` | Implemented |
| Embedded interpreter (wasmi-based) | `rvf-wasm` | Future |
| Combined interpreter+microkernel build | `rvf-wasm` | Future |
