# rvf-ebpf

Real eBPF program compiler and embedder for RVF cognitive containers.

## What It Does

`rvf-ebpf` compiles real BPF C programs with `clang` and embeds them into `.rvf` files as `EBPF_SEG` segments. These programs provide kernel-level acceleration for vector operations.

## Included Programs

| Program | Type | Description |
|---------|------|-------------|
| `xdp_distance.c` | XDP | L2 vector distance computation with LRU vector cache using BPF maps |
| `socket_filter.c` | Socket Filter | Port-based allow-list access control with per-CPU counters |
| `tc_query_route.c` | TC Classifier | Query priority routing (hot/warm/cold traffic classes) |

## Usage

```rust
use rvf_ebpf::{EbpfCompiler, programs};

// Access real BPF C source
println!("{}", programs::XDP_DISTANCE);
println!("{}", programs::SOCKET_FILTER);
println!("{}", programs::TC_QUERY_ROUTE);

// Compile with clang (requires clang installed)
let compiler = EbpfCompiler::new()?;
let program = compiler.compile_source(
    programs::SOCKET_FILTER,
    EbpfProgramType::SocketFilter,
)?;

// Embed compiled ELF into RVF
store.embed_ebpf(
    program.program_type as u8,
    program.attach_type as u8,
    1536,
    &program.elf_bytes,
    program.btf_bytes.as_deref(),
)?;
```

## Requirements

- `clang` with BPF target support (for compilation)
- Programs can also be pre-compiled and embedded as raw ELF bytes

## Tests

```bash
cargo test -p rvf-ebpf  # 17 tests
```

## License

MIT OR Apache-2.0
