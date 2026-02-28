# ADR-030: RVF Cognitive Container -- Self-Booting Vector Files

**Status**: Proposed
**Date**: 2026-02-14
**Authors**: ruv.io, RuVector Architecture Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow
**Depends on**: ADR-029 (RVF canonical format), ADR-005 (WASM runtime), ADR-012 (Security remediation)

## Context

### The Passive Data Problem

RVF today is a sophisticated binary format for vector data: it carries embeddings, HNSW indexes, quantization codebooks, cryptographic signatures, and a 5.5 KB WASM microkernel. But it remains fundamentally passive. To serve queries, an external runtime must:

1. Parse the file
2. Load the manifest
3. Build in-memory indexes
4. Expose an API (HTTP, gRPC, or in-process)
5. Manage lifecycle, health checks, and scaling

This external dependency chain creates friction in four critical deployment scenarios:

**Confidential Computing (TEE enclaves)**: Today, deploying vector search inside an SGX/SEV-SNP/TDX enclave requires installing a full runtime inside the enclave, increasing the Trusted Computing Base (TCB) and attack surface. The WITNESS_SEG attestation model (ADR-029) records proofs of TEE execution, but the runtime itself is not attested -- only the data is. A self-booting file that carries its own verified kernel eliminates the unattested runtime gap.

**Serverless vector search**: Lambda-style platforms cold-start a runtime, deserialize the index, and then serve queries. For RVF files under 10 MB, the runtime overhead dominates: Firecracker boots in ~125 ms, but the Node.js/Python runtime on top adds 500-2000 ms. If the .rvf file IS the microservice -- booting directly into a query-serving kernel -- cold start collapses to the Firecracker boot window alone.

**Air-gapped / edge deployment**: Edge nodes in disconnected environments (submarines, satellites, field hospitals, industrial control) cannot rely on package managers or container registries. A single file that self-boots and serves queries removes all host dependencies beyond a hypervisor or bare metal.

**Portable compute**: The AppImage model (self-contained Linux application in a single file) proves that users prefer "download, chmod +x, run." RVF should offer the same experience for vector search: drop a file, it runs.

### Why WASM Alone Is Insufficient

The existing WASM_SEG (Tier 1) provides portable compute at 5.5 KB, but WASM has structural limitations for the scenarios above:

- **No direct hardware access**: WASM cannot bind to NVMe, network interfaces, or TEE hardware without a host runtime.
- **No kernel services**: WASM lacks syscalls for file I/O, networking, memory-mapped I/O, and signal handling.
- **No attestation binding**: WASM modules cannot generate or verify TEE attestation quotes.
- **Performance ceiling**: WASM's linear memory model and lack of SIMD beyond v128 limits throughput for large-scale vector operations.
- **WASI is not yet sufficient**: WASI Preview 2 (stabilized early 2024) covers basic I/O, but WASI 0.3 completion is tracked on the WASI roadmap (wasi.dev/roadmap) with previews available as of early 2026; it still lacks TEE integration, direct device access, and the networking primitives needed for a standalone query server.

These limitations motivate a complementary execution tier that provides kernel-level capabilities while preserving WASM's portability for constrained environments.

## State of the Art

### Rust Unikernels

**Hermit OS / RustyHermit** (RWTH Aachen): A Rust-native unikernel where the application links directly against the kernel library. The kernel supports x86_64, aarch64, and riscv64 targets. The entire kernel is written in Rust with zero C/C++ dependencies, making it composable with Rust applications at link time. Hermit runs on QEMU/KVM, Firecracker, and Uhyve (a custom lightweight VMM). The kernel binary for a minimal application is approximately 200-400 KB compressed, well within the RVF segment budget.

**Theseus OS** (Rice/Yale): A safe-language OS using Rust's ownership model for isolation instead of hardware privilege rings. Runs in a single address space and single privilege level. While not production-ready, its cell-based architecture demonstrates that Rust's type system can enforce kernel-level isolation without MMU overhead -- relevant for TEE enclaves where virtual memory is constrained.

**Asterinas** (USENIX ATC 2025): A Linux ABI-compatible framekernel written in Rust, supporting 230+ syscalls. Its "OSTD" framework confines unsafe Rust to ~14% of the codebase. Asterinas proves that a Rust kernel can achieve Linux-comparable performance while maintaining memory safety guarantees. Its Linux ABI compatibility means existing Rust binaries can run unmodified.

**RuxOS** (syswonder): A modular Rust unikernel that selectively includes only the OS components an application needs, achieving minimal image sizes. Supports multiple architectures including x86_64, aarch64, and riscv64.

### MicroVM Technology

**Firecracker** (AWS, ~50K lines of Rust): Purpose-built for serverless. Achieves <125 ms boot time to init process with <5 MiB memory footprint. Powers AWS Lambda and Fargate. Written entirely in Rust. The minimal attack surface (no USB, no GPU passthrough, no legacy devices) makes it ideal for running untrusted workloads. Firecracker accepts a kernel image + rootfs as inputs -- exactly what a KERNEL_SEG would provide.

**Cloud Hypervisor** (Intel/ARM): A Rust-based VMM targeting cloud workloads. Supports VirtIO devices, VFIO passthrough, and live migration. More feature-rich than Firecracker but larger attack surface.

**Uhyve** (Hermit project): A minimal hypervisor specifically designed for Hermit unikernels. Even faster boot times than Firecracker for single-application workloads because it skips BIOS/UEFI boot and loads the unikernel directly.

### eBPF for Data Processing

**eBPF architecture**: Programs run in the Linux kernel's BPF virtual machine with a JIT compiler producing native code. Programs are verified before execution (no loops, bounded execution, memory safety). Map types (hash tables, arrays, ring buffers, LPM tries) provide shared state between kernel and userspace.

**Aya (Rust eBPF framework)**: Pure Rust eBPF development with BTF (BPF Type Format) support for cross-kernel portability. No C toolchain required. Compiles eBPF programs from Rust source. Supports XDP (eXpress Data Path), TC (Traffic Control), tracepoints, kprobes, and socket filters. FOSDEM 2025 featured sessions on building production eBPF systems with Aya.

**Relevance to vector search**: eBPF programs attached at XDP or TC hooks can perform distance computations on incoming query packets before they reach userspace, reducing round-trip latency. BPF maps can hold hot vectors (top-accessed embeddings) in kernel memory, serving as a kernel-level L0 cache. This maps directly to RVF's temperature tiering model (HOT_SEG).

### Confidential Computing Runtimes

**Enarx** (Confidential Computing Consortium): A deployment platform for WebAssembly inside TEEs. First open-source project donated to the CCC. Supports SGX and SEV-SNP. Uses WASM as the execution format inside the enclave.

**Gramine** (formerly Graphene-SGX): A library OS that runs unmodified Linux applications inside SGX enclaves. Adds ~100-200 KB to the TCB. Widely used in production confidential computing deployments.

**Occlum**: An SGX library OS supporting multi-process, multi-threaded applications. Provides a POSIX-compatible API inside the enclave.

**Key insight**: All current CC runtimes add a separate library OS layer. If the RVF file carries its own kernel that IS the library OS, the TCB shrinks to just the kernel image (which is cryptographically measured) plus the TEE hardware. The attestation quote then covers both data and runtime in a single measurement.

**2025 developments**: The Confidential Computing Consortium's 2025 survey found that CC has become foundational for data-centric innovation, but implementation complexity hinders adoption. Self-booting RVF files address this directly: the deployment complexity collapses to "transfer file, boot."

### Self-Executing Archive Formats

**AppImage**: Self-mounting disk images using FUSE. Users download, `chmod +x`, and run. No installation. The model proves that single-file deployment works at scale for Linux desktop applications.

**binctr** (Jessie Frazelle): Fully static, unprivileged, self-contained containers as executable binaries. Embeds a compressed rootfs inside the binary. Demonstrates that a useful runtime environment can fit in a single self-extracting file.

**Snap / Flatpak**: More complex than AppImage but demonstrate sandboxed execution from self-contained bundles.

**Key gap**: None of these formats are designed for compute workloads that serve network APIs. They all target desktop applications. RVF's KERNEL_SEG fills this gap: a self-booting file that starts a query server, not a GUI.

### WebAssembly System Interface (WASI)

**WASI Preview 2** (stabilized early 2024): Covers basic file I/O, clocks, random, stdout/stderr. The Component Model enables composing WASM modules with typed interfaces (WIT).

**WASI 0.3** (tracked on wasi.dev/roadmap, previews available as of early 2026): Adds native async support with the Component Model. Still lacks TEE integration, direct network socket creation, and the system-level primitives needed for a standalone service.

**WebAssembly vs. Unikernels** (arxiv 2509.09400): A comparative study found that WASM offers lower cold-start latency for lightweight functions but degrades with complex workloads and I/O operations, while Firecracker-based MicroVMs provide stable, higher performance for I/O-heavy tasks like vector search. This validates the three-tier model: WASM for lightweight edge, unikernel for production workloads.

### Post-Quantum Cryptography for Kernel Signing

**ML-DSA-65** (FIPS 204): Module-lattice-based digital signatures at NIST Security Level 3. Multiple pure Rust implementations exist (fips204, ml-dsa, libcrux-ml-dsa crates). The fips204 crate operates in constant time, is `#[no_std]` compatible, has no heap allocations, and exposes the RNG -- suitable for bare-metal and TEE environments. RVF already uses ML-DSA-65 for segment signing in CRYPTO_SEG; the same infrastructure extends naturally to KERNEL_SEG signing.

## Decision

### Add KERNEL_SEG (0x0E) and EBPF_SEG (0x0F) to the RVF segment type registry

Extend the RVF format with two new segment types that embed executable compute alongside vector data, creating a three-tier execution model:

| Tier | Segment | Size | Target Environment | Boot Time |
|------|---------|------|--------------------|-----------|
| **1: WASM** | WASM_SEG (exists) | 5.5 KB | Browser, edge, IoT, Cognitum tiles | <1 ms (instantiate) |
| **2: eBPF** | EBPF_SEG (0x0F) | 10-50 KB | Linux kernel fast path, XDP, TC | <20 ms (load + verify) |
| **3: Unikernel** | KERNEL_SEG (0x0E) | 200 KB - 2 MB | TEE enclaves, Firecracker, bare metal | <125 ms (full boot) |

### Tier Selection Logic

```
if target == browser || target == wasm_runtime {
    use WASM_SEG (Tier 1)
} else if linux_kernel_available && query_is_hot_path {
    use EBPF_SEG (Tier 2)  // kernel-level L0 cache
} else if tee_required || standalone_service {
    use KERNEL_SEG (Tier 3) // self-booting
} else {
    use host runtime (existing behavior)
}
```

An RVF file MAY contain segments from multiple tiers simultaneously. A file with WASM_SEG + KERNEL_SEG can serve queries from a browser (Tier 1) or boot as a standalone service (Tier 3) from the same file.

## KERNEL_SEG Wire Format

### Segment Header

KERNEL_SEG uses the standard 64-byte SegmentHeader (ADR-029) with `seg_type = 0x0E`. The payload begins with a KernelHeader followed by the compressed kernel image.

### KernelHeader (128 bytes, repr(C))

```
Offset  Size  Field             Description
------  ----  -----             -----------
0x00    4     kernel_magic      Magic: 0x52564B4E ("RVKN")
0x04    2     header_version    KernelHeader format version (currently 1)
0x06    1     arch              Target architecture enum
0x07    1     kernel_type       Kernel type enum
0x08    4     kernel_flags      Bitfield flags
0x0C    4     min_memory_mb     Minimum RAM required (MiB)
0x10    8     entry_point       Virtual address of kernel entry point
0x18    8     image_size        Uncompressed kernel image size (bytes)
0x20    8     compressed_size   Compressed kernel image size (bytes)
0x28    1     compression       Compression algorithm (same as SegmentHeader)
0x29    1     api_transport     API transport enum
0x2A    2     api_port          Default API port (network byte order)
0x2C    4     api_version       Supported RVF query API version
0x30    32    image_hash        SHAKE-256-256 of uncompressed kernel image
0x50    16    build_id          Unique build identifier (UUID v7)
0x60    8     build_timestamp   Build time (nanosecond UNIX timestamp)
0x68    4     vcpu_count        Recommended vCPU count (0 = single)
0x6C    4     reserved_0        Reserved (must be zero)
0x70    8     cmdline_offset    Offset to kernel command line within payload
0x78    4     cmdline_length    Length of kernel command line (bytes)
0x7C    4     reserved_1        Reserved (must be zero)
```

### Architecture Enum (u8)

```
Value  Name       Description
-----  ----       -----------
0x00   x86_64     AMD64 / Intel 64
0x01   aarch64    ARM 64-bit (ARMv8-A and later)
0x02   riscv64    RISC-V 64-bit (RV64GC)
0xFE   universal  Architecture-independent (e.g., interpreted)
0xFF   unknown    Reserved / unspecified
```

### Kernel Type Enum (u8)

```
Value  Name            Description
-----  ----            -----------
0x00   hermit          Hermit OS unikernel (Rust-native)
0x01   micro_linux     Minimal Linux kernel (bzImage compatible)
0x02   asterinas       Asterinas framekernel (Linux ABI compatible)
0x03   wasi_preview2   WASI Preview 2 component (alternative to WASM_SEG)
0x04   custom          Custom kernel (requires external VMM knowledge)
0xFE   test_stub       Test stub for CI (boots, reports health, exits)
0xFF   reserved        Reserved
```

### Kernel Flags (u32 bitfield)

```
Bit   Name                 Description
---   ----                 -----------
0     REQUIRES_TEE         Kernel must run inside a TEE enclave
1     REQUIRES_KVM         Kernel requires KVM (hardware virtualization)
2     REQUIRES_UEFI        Kernel requires UEFI boot (not raw bzImage)
3     HAS_NETWORKING       Kernel includes network stack
4     HAS_QUERY_API        Kernel exposes RVF query API on api_port
5     HAS_INGEST_API       Kernel exposes RVF ingest API
6     HAS_ADMIN_API        Kernel exposes health/metrics API
7     ATTESTATION_READY    Kernel can generate TEE attestation quotes
8     SIGNED               Kernel image is signed (SignatureFooter follows)
9     MEASURED             Kernel measurement stored in WITNESS_SEG
10    COMPRESSED           Image is compressed (per compression field)
11    RELOCATABLE          Kernel is position-independent
12    HAS_VIRTIO_NET       Kernel includes VirtIO network driver
13    HAS_VIRTIO_BLK       Kernel includes VirtIO block driver
14    HAS_VSOCK            Kernel includes VSOCK for host communication
15-31 reserved             Reserved (must be zero)
```

### API Transport Enum (u8)

```
Value  Name       Description
-----  ----       -----------
0x00   tcp_http   HTTP/1.1 over TCP (default)
0x01   tcp_grpc   gRPC over TCP (HTTP/2)
0x02   vsock      VirtIO socket (Firecracker host<->guest)
0x03   shared_mem Shared memory region (for same-host co-location)
0xFF   none       No network API (batch mode only)
```

### Payload Layout

```
[SegmentHeader: 64 bytes]
[KernelHeader: 128 bytes]
[Kernel command line: cmdline_length bytes, NUL-terminated, padded to 8-byte boundary]
[Compressed kernel image: compressed_size bytes]
[Optional: SignatureFooter if SIGNED flag is set]
```

The kernel image is compressed with the algorithm specified in `KernelHeader.compression`. ZSTD is the recommended default for kernel images due to its high compression ratio at fast decompression speeds (~1.5 GB/s). A 400 KB Hermit unikernel compresses to approximately 150-200 KB with ZSTD level 3.

### Signing

When the `SIGNED` flag is set, a SignatureFooter (identical to the existing RVF SignatureFooter format) is appended after the compressed kernel image. The signature covers the concatenation of:

```
signed_data = KernelHeader || cmdline_bytes || compressed_image
```

The same ML-DSA-65 or Ed25519 keys used for CRYPTO_SEG segment signing can sign KERNEL_SEG. This means a single key pair attests both the data and the runtime, providing end-to-end integrity from a single trust root.

## EBPF_SEG Wire Format

### EbpfHeader (64 bytes, repr(C))

```
Offset  Size  Field             Description
------  ----  -----             -----------
0x00    4     ebpf_magic        Magic: 0x52564250 ("RVBP")
0x04    2     header_version    EbpfHeader format version (currently 1)
0x06    1     program_type      eBPF program type enum
0x07    1     attach_type       eBPF attach point enum
0x08    4     program_flags     Bitfield flags
0x0C    2     insn_count        Number of BPF instructions (max 65535)
0x0E    2     max_dimension     Maximum vector dimension this program handles
0x10    8     program_size      ELF object size (bytes)
0x18    4     map_count         Number of BPF maps defined
0x1C    4     btf_size          BTF (BPF Type Format) section size
0x20    32    program_hash      SHAKE-256-256 of the ELF object
```

### eBPF Program Type Enum (u8)

```
Value  Name              Description
-----  ----              -----------
0x00   xdp_distance      XDP program for distance computation on packets
0x01   tc_filter         TC classifier for query routing
0x02   socket_filter     Socket filter for query preprocessing
0x03   tracepoint        Tracepoint for performance monitoring
0x04   kprobe            Kprobe for dynamic instrumentation
0x05   cgroup_skb        Cgroup socket buffer filter
0xFF   custom            Custom program type
```

### eBPF Attach Type Enum (u8)

```
Value  Name              Description
-----  ----              -----------
0x00   xdp_ingress       XDP hook on NIC ingress
0x01   tc_ingress        TC ingress qdisc
0x02   tc_egress         TC egress qdisc
0x03   socket_filter     Socket filter attachment
0x04   cgroup_ingress    Cgroup ingress
0x05   cgroup_egress     Cgroup egress
0xFF   none              No automatic attachment
```

### Payload Layout

```
[SegmentHeader: 64 bytes]
[EbpfHeader: 64 bytes]
[BPF ELF object: program_size bytes]
[BTF section: btf_size bytes (if btf_size > 0)]
[Map definitions: map_count * 32 bytes]
[Optional: SignatureFooter if SIGNED flag in SegmentHeader]
```

## Execution Model

### Tier 1: WASM Microkernel (Existing)

No changes. The existing 5.5 KB WASM microkernel in WASM_SEG continues to serve as the portable compute layer for browsers, edge devices, and Cognitum tiles. WASM provides the widest deployment reach with the smallest footprint.

### Tier 2: eBPF Fast Path

The eBPF tier accelerates the hot path for Linux-hosted deployments:

1. **Loader** reads EBPF_SEG from the RVF file.
2. **Verifier** (kernel BPF verifier) validates the program.
3. **JIT** compiles to native code.
4. **Attach** to the specified hook point (XDP, TC, tracepoint).
5. **BPF maps** are populated with hot vectors from HOT_SEG.
6. **Query path**: Incoming packets hit the XDP/TC program, which computes distances against the hot vector cache in BPF map memory. Queries that can be satisfied from the hot cache return immediately from kernel space (bypassing all userspace overhead). Cache misses are passed to userspace for full HNSW traversal.

This creates a two-level query architecture:
- **L0 (kernel)**: eBPF program + BPF map hot cache. Sub-microsecond for cache hits.
- **L1 (userspace)**: Full RVF runtime for cache misses. Standard HNSW latency.

The temperature model in SKETCH_SEG determines which vectors are promoted to the eBPF L0 cache.

### Tier 3: Unikernel Self-Boot

The unikernel tier makes the RVF file a self-contained microservice:

1. **Launcher** (rvf-launch CLI or library) reads the KERNEL_SEG and MANIFEST_SEG.
2. **Decompression**: The ZSTD-compressed kernel image is decompressed.
3. **Verification**: The kernel image hash is verified against `KernelHeader.image_hash`. If SIGNED, the SignatureFooter is verified. If MEASURED, the measurement in WITNESS_SEG is cross-checked.
4. **VMM setup**: Firecracker (or Uhyve, or Cloud Hypervisor) is configured:
   - vCPUs: `KernelHeader.vcpu_count` (default 1)
   - Memory: `KernelHeader.min_memory_mb` (default 32 MiB)
   - Kernel: decompressed image
   - Boot args: kernel command line from payload
   - Block device: the .rvf file itself (read-only virtio-blk)
   - Network: virtio-net or vsock per `api_transport`
5. **Boot**: The VMM starts the kernel. The unikernel:
   a. Initializes with a minimal runtime (no init system, no systemd).
   b. Memory-maps the .rvf file from the virtio-blk device.
   c. Reads the Level 0 manifest (4 KB at EOF) for instant hotset access.
   d. Starts the query API on the configured port/transport.
   e. Begins background progressive index loading (Level 1, Layer B, Layer C).
6. **Ready signal**: The kernel sends a health check response on `api_port`, or a VSOCK notification to the host.

**Boot timeline (target)**:

```
T+0 ms       VMM creates microVM
T+5 ms       Kernel image loaded into guest memory
T+50 ms      Kernel init complete, virtio drivers up
T+55 ms      .rvf file memory-mapped, Level 0 parsed
T+60 ms      Hot cache loaded, entry points available
T+80 ms      Query API listening on api_port
T+125 ms     Ready signal sent to host
T+500 ms     Layer B loaded (background)
T+2000 ms    Layer C loaded, full recall available
```

### Minimum Viable Kernel Profile

The first bootable KERNEL_SEG MUST implement only:

1. **Read-only query API** — k-NN search over embedded vectors
2. **Health endpoint** — Returns 200 when boot is complete and index is loaded
3. **Metrics read** — Basic counters (queries served, latency p50/p99, uptime)

Excluded from the minimum profile (added via KernelFlags):
- Ingest (live vector insertion) — requires `INGEST_ENABLED` flag
- Admin API (compaction, config changes) — requires `ADMIN_ENABLED` flag
- Streaming protocol — requires `STREAMING_ENABLED` flag

This ensures the smallest possible TCB for the initial bootable artifact. Ingest into a self-booting RVF is handled by default via a separate signed update segment (OVERLAY_SEG), not live mutation inside the microVM. Live ingest may be enabled explicitly when the deployment model requires it.

### Cross-Tier Cooperation

A single RVF file can embed all three tiers. The runtime selects the appropriate tier based on the deployment context:

```
.rvf file
  |
  +-- WASM_SEG   -> Browser / IoT / tile  (always available)
  +-- EBPF_SEG   -> Linux kernel fast path (optional, requires CAP_BPF)
  +-- KERNEL_SEG -> Self-booting service   (optional, requires VMM)
  +-- VEC_SEG    -> Vector data            (always present)
  +-- INDEX_SEG  -> HNSW index             (always present)
  +-- ...other segments as needed
```

On a Linux host with Firecracker, the launcher can:
1. Boot the KERNEL_SEG as a microVM.
2. Load EBPF_SEG into the host kernel for the L0 hot cache.
3. Route queries: eBPF handles hot-path hits, microVM handles misses.

In a browser, only the WASM_SEG is used; KERNEL_SEG and EBPF_SEG are ignored.

### Authority Boundary: Host eBPF vs. Guest Kernel

When Tier 2 (eBPF) and Tier 3 (unikernel) operate simultaneously on the same file:

- The **guest kernel** is the authoritative query engine. It owns authentication, rate limiting, audit logging, and witness chain emission.
- The **host eBPF** is an acceleration layer only. It serves cache hits from BPF maps but MUST NOT finalize results without a guest-signed witness record.
- For cache misses, the eBPF program forwards the query to the guest via virtio-vsock. The guest computes the result, emits a witness entry, and returns the response.
- The eBPF program MUST NOT emit witness entries or modify the witness chain.

This rule prevents split-brain policies and ensures a single complete audit trail regardless of which tier served the query.

## Security Model

### Kernel Image Integrity

Every KERNEL_SEG image MUST be integrity-protected by at least one of:

1. **Content hash** (mandatory): `KernelHeader.image_hash` contains the SHAKE-256-256 digest of the uncompressed kernel image. The launcher verifies this before booting.
2. **Cryptographic signature** (recommended): A SignatureFooter with ML-DSA-65 or Ed25519 over the kernel header + command line + compressed image.
3. **TEE measurement** (for confidential computing): A `MEASURED` WITNESS_SEG record containing the kernel's expected measurement (MRENCLAVE for SGX, launch digest for SEV-SNP/TDX).

### Attestation Binding (KERNEL_SEG + WITNESS_SEG)

For confidential computing deployments, KERNEL_SEG and WITNESS_SEG cooperate:

```
KERNEL_SEG:
  image_hash = H(kernel_image)
  flags: REQUIRES_TEE | ATTESTATION_READY | MEASURED

WITNESS_SEG (witness_type = 0x10, KERNEL_ATTESTATION):
  measurement:   Expected TEE measurement of the kernel
  nonce:         Anti-replay nonce
  sig_key_id:    Reference to signing key in CRYPTO_SEG
  evidence:      Platform-specific attestation quote

Verification chain:
  1. Verify KERNEL_SEG.image_hash matches H(decompressed image)
  2. Verify KERNEL_SEG SignatureFooter against CRYPTO_SEG key
  3. Boot kernel inside TEE
  4. Kernel generates attestation quote
  5. Verify quote.measurement == WITNESS_SEG.measurement
  6. Verify quote.measurement == H(loaded kernel image)
  -> Data + runtime + TEE form a single measured trust chain
```

### Verification Algorithm

A compliant launcher MUST execute these steps in order, failing closed on any error:

1. Read KERNEL_SEG header. Decompress kernel image.
2. Compute SHAKE-256-256 of decompressed bytes. Compare to `image_hash`. **FAIL** if mismatch.
3. If `SIGNED` flag is set: locate SignatureFooter. Verify signature over (KernelHeader || compressed_image). **FAIL** if signature missing or invalid.
4. If `SIGNED` flag is NOT set but launcher policy requires signing: **FAIL** (refuse unsigned kernels in production).
5. If `REQUIRES_TEE` flag is set: verify current environment is a TEE. **FAIL** if running outside enclave/VM.
6. If `MEASURED` flag is set: locate corresponding WITNESS_SEG record with `witness_type = KERNEL_ATTESTATION (0x10)`. Verify `action_hash` matches `image_hash`. **FAIL** if no matching witness or hash mismatch.
7. Boot kernel. Wait for health endpoint. **FAIL** if health not ready within boot timeout.

Failure at any step is fatal. The launcher MUST NOT serve queries from an unverified kernel.

### eBPF Safety

eBPF programs in EBPF_SEG are verified by the Linux kernel's BPF verifier before execution. This provides:

- **Termination guarantee**: No unbounded loops.
- **Memory safety**: All memory accesses are bounds-checked.
- **Privilege separation**: Programs run with restricted capabilities.
- **No kernel crashes**: A verified eBPF program cannot panic or fault the kernel.

Additionally, EBPF_SEG images are hash-verified (`EbpfHeader.program_hash`) and optionally signed, preventing injection of malicious programs.

### eBPF Dimension Constraint

The `max_dimension` field in EbpfHeader declares the maximum vector dimension the program can process. The eBPF verifier requires bounded loops, so each distance computation program is compiled for a fixed maximum dimension.

The loader MUST reject an EBPF_SEG whose `max_dimension` is less than the file's vector dimension. This prevents loading incompatible programs that would produce incorrect results or verifier failures.

Recommended maximum: 2048 dimensions per eBPF program. For higher dimensions, use Tier 1 (WASM) or Tier 3 (unikernel) which have no loop bound constraints.

### Sandbox Boundaries

| Tier | Sandbox | Escape Risk | Mitigation |
|------|---------|-------------|------------|
| WASM | WASM VM (linear memory) | Very Low | Proven isolation model |
| eBPF | BPF verifier + JIT | Very Low | Kernel-enforced bounds |
| Unikernel | VMM (Firecracker/KVM) | Low | Hardware virtualization (VT-x/AMD-V) |
| TEE | Hardware enclave | Very Low | Silicon-level isolation |

### Supply Chain

Kernel images in KERNEL_SEG SHOULD be reproducibly built. The `build_id` (UUID v7) and `build_timestamp` enable tracing a kernel image back to its exact source revision and build environment. Signing with ML-DSA-65 provides post-quantum resistance for the kernel supply chain.

### Reference Implementation

The reference kernel type is **Hermit OS** (https://hermit-os.org/). The build pipeline:

1. Source: `hermit-os/kernel` repository at a pinned git tag
2. Build: `cargo build --target x86_64-unknown-hermit --release`
3. Link: Application (`rvf-runtime` compiled as unikernel) links against Hermit kernel library
4. Compress: `zstd -19` on the resulting ELF binary
5. Embed: `rvf embed-kernel --arch x86_64 --type hermit mydata.rvf`

The build MUST be reproducible: same source + same Rust toolchain = identical `image_hash`. This is enforced by pinning the Rust toolchain version in `rust-toolchain.toml` and recording the `build_id` (UUID v7) in KernelHeader.

### Signing Algorithm Selection

| Context | Algorithm | Rationale |
|---------|-----------|-----------|
| Developer iteration, CI builds | Ed25519 | Fast (us), small signatures (64 bytes), existing key infrastructure |
| Published releases, public distribution | ML-DSA-65 (FIPS 204) | Post-quantum resistance, NIST standardized |
| Migration period | Dual (Ed25519 + ML-DSA-65) | SignatureFooter supports a signature list; verifiers accept either |
| After cutover (configurable date) | ML-DSA-65 only | Files with `REQUIRES_PQ` flag reject Ed25519-only signatures |

This matches ADR-029's key authority model and ensures backward compatibility during the post-quantum transition.

## Backward Compatibility

### KERNEL_SEG and EBPF_SEG are fully optional

Files without these segments work exactly as they do today. The new segment types use previously unassigned discriminator values (0x0E and 0x0F), which existing readers will skip as unknown segments per the RVF forward-compatibility rule: "Unknown segment types MUST be skipped by readers that do not understand them."

### Level 0 Root Manifest Extension

The Level0Root reserved area (offset 0xF00, 252 bytes) contains a KernelPtr (16 bytes) at offset 0xF44:

```
Offset  Size  Field               Description
------  ----  -----               -----------
0xF44   8     kernel_seg_offset   Byte offset to first KERNEL_SEG (0 if absent)
0xF4C   4     kernel_seg_length   Byte length of KERNEL_SEG payload
0xF50   4     kernel_flags_hint   Copy of KernelHeader.kernel_flags for fast scanning
```

Old readers see zeros at these offsets and continue working normally. New readers check `kernel_seg_offset != 0` to determine if the file is self-booting.

### SegmentType Registry Update

All computational segment types are now implemented in `rvf-types/src/segment_type.rs`:

```rust
#[repr(u8)]
pub enum SegmentType {
    // ... existing types 0x00 - 0x0D ...
    /// Embedded kernel / unikernel image for self-booting.
    Kernel = 0x0E,
    /// Embedded eBPF program for kernel fast path.
    Ebpf = 0x0F,
    /// Embedded WASM bytecode for self-bootstrapping execution.
    Wasm = 0x10,
    // ... COW segments 0x20-0x23 (ADR-031) ...
    // ... Domain expansion segments 0x30-0x32 ...
}
```

The full registry (23 types) is documented in ADR-029. Available ranges: 0x11-0x1F, 0x24-0x2F, 0x33-0xEF. Values 0xF0-0xFF remain reserved.

## Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| KERNEL_SEG decompression | <10 ms for 2 MB image | ZSTD streaming decompression benchmark |
| Firecracker boot to init | <50 ms | Firecracker metrics (API socket ready) |
| Kernel init to API ready | <75 ms | Time from init to first successful health check |
| Total cold start (file to API) | <125 ms | End-to-end: read segment, decompress, boot, serve |
| First query after boot | <200 ms | Time to first non-error query response |
| Full recall available | <2 s | Time until Layer C loaded and recall@10 >= 0.95 |
| eBPF load + verify | <20 ms | Time from read to attached + serving |
| eBPF hot-path query | <10 us | BPF map lookup + distance compute |
| Kernel image size (Hermit) | <400 KB uncompressed | Minimal query-serving unikernel |
| Kernel image size (micro-Linux) | <2 MB uncompressed | bzImage with minimal initramfs |
| KERNEL_SEG overhead | <200 KB compressed | ZSTD level 3 on Hermit image |
| Memory footprint (unikernel) | <32 MiB | Firecracker guest memory for 1M vectors |

## Implementation Phases

### Phase 1: Segment Types and Headers (rvf-types)

**Duration**: 1 week
**Status**: **Complete** (as of 2026-02-16)

**Implementation notes**:
- `SegmentType::Kernel = 0x0E`, `SegmentType::Ebpf = 0x0F`, and `SegmentType::Wasm = 0x10` are all defined in `rvf-types/src/segment_type.rs` with `TryFrom<u8>` round-trip support and unit tests.
- The `rvf-runtime` write path (`write_path.rs`) implements `write_kernel_seg()` and `write_ebpf_seg()` methods that accept raw header byte arrays, with round-trip tests.
- **`KernelHeader`** (128-byte `repr(C)` struct) is fully implemented in `rvf-types/src/kernel.rs` with:
  - `KernelArch` enum (X86_64, Aarch64, Riscv64, Universal, Unknown) with `TryFrom<u8>`
  - `KernelType` enum (Hermit, MicroLinux, Asterinas, WasiPreview2, Custom, TestStub) with `TryFrom<u8>`
  - `ApiTransport` enum (TcpHttp, TcpGrpc, Vsock, SharedMem, None) with `TryFrom<u8>`
  - 15 `KERNEL_FLAG_*` bitfield constants (bits 0-14)
  - `to_bytes()` / `from_bytes()` serialization with compile-time size assertion
  - 12 tests: header size, magic, round-trip, bad magic, field offsets, enum round-trips, flag bit positions, api_port network byte order, reserved field zeroing
- **`EbpfHeader`** (64-byte `repr(C)` struct) is fully implemented in `rvf-types/src/ebpf.rs` with:
  - `EbpfProgramType` enum (XdpDistance, TcFilter, SocketFilter, Tracepoint, Kprobe, CgroupSkb, Custom) with `TryFrom<u8>`
  - `EbpfAttachType` enum (XdpIngress, TcIngress, TcEgress, SocketFilter, CgroupIngress, CgroupEgress, None) with `TryFrom<u8>`
  - `to_bytes()` / `from_bytes()` serialization with compile-time size assertion
  - 10 tests: header size, magic, round-trip, bad magic, field offsets, enum round-trips, max_dimension, large program size
- **`WasmHeader`** (64-byte `repr(C)` struct) is fully implemented in `rvf-types/src/wasm_bootstrap.rs` with:
  - `WasmRole` enum (Microkernel, Interpreter, Combined, Extension, ControlPlane) with `TryFrom<u8>`
  - `WasmTarget` enum (Wasm32, WasiP1, WasiP2, Browser, BareTile) with `TryFrom<u8>`
  - 8 `WASM_FEAT_*` bitfield constants
  - `to_bytes()` / `from_bytes()` serialization with compile-time size assertion
  - 10 tests
- All types are exported from `rvf-types/src/lib.rs`.

**Deliverables**:
- [x] Add `Kernel = 0x0E` and `Ebpf = 0x0F` to `SegmentType` enum
- [x] Add `Wasm = 0x10` to `SegmentType` enum
- [x] Define `KernelHeader` (128-byte repr(C) struct) with compile-time size assertion
- [x] Define `EbpfHeader` (64-byte repr(C) struct) with compile-time size assertion
- [x] Define `WasmHeader` (64-byte repr(C) struct) with compile-time size assertion
- [x] Define architecture, kernel type, transport, and program type enums
- [x] Define kernel flags (15 bits) and WASM feature flags (8 bits)
- [ ] Add `KernelPtr` to Level0Root reserved area
- [x] Unit tests for all new types, field offsets, and round-trips (32+ tests)

**Preconditions**: rvf-types crate exists and compiles (satisfied)
**Success criteria**: `cargo test -p rvf-types` passes, all new structs have offset tests -- **MET**

### Phase 2: eBPF Program Embedding + Extraction (rvf-ebpf)

**Duration**: 2 weeks
**Deliverables**:
- New crate `rvf-ebpf` with EBPF_SEG codec (read/write)
- BPF ELF parser (extract program, maps, BTF sections)
- Integration with Aya for program loading and map population
- Hot vector cache loader (HOT_SEG vectors into BPF hash map)
- XDP distance computation program template (L2, cosine)
- Integration test: load EBPF_SEG, attach to test interface, verify distance computation

**Preconditions**: Phase 1 complete, Linux kernel >= 5.15 for BTF support
**Success criteria**: eBPF program loads from EBPF_SEG, computes correct L2 distances on test packets

### Phase 3: Hermit/RustyHermit Unikernel Integration (rvf-kernel)

**Duration**: 3 weeks
**Deliverables**:
- New crate `rvf-kernel` with KERNEL_SEG codec (read/write)
- Hermit-based query server application (links against hermit-kernel)
  - VirtIO block driver for reading .rvf file
  - Minimal HTTP server (query + health endpoints)
  - RVF manifest parser and progressive loader
  - Distance computation using Hermit's SIMD support
- KERNEL_SEG builder: compile Hermit app, ZSTD compress, embed in segment
- KERNEL_SEG extractor: read segment, verify hash, decompress
- CI build pipeline for Hermit kernel images (x86_64, aarch64)

**Preconditions**: Phase 1 complete, Hermit toolchain set up
**Success criteria**: Hermit kernel image < 400 KB, compresses to < 200 KB, boots in QEMU

### Phase 4: Firecracker Launcher (rvf-launch)

**Duration**: 2 weeks
**Deliverables**:
- New crate `rvf-launch` (CLI + library)
- Firecracker microVM configuration generator
- Kernel extraction, decompression, and verification pipeline
- VirtIO block device setup (pass .rvf file as read-only disk)
- Network configuration (virtio-net or vsock)
- Health check polling (wait for API ready signal)
- Graceful shutdown (SIGTERM to microVM)
- CLI: `rvf launch mydata.rvf` -- boots and serves
- Integration test: launch .rvf in Firecracker, query via HTTP, verify results

**Preconditions**: Phase 3 complete, Firecracker binary available
**Success criteria**: `rvf launch` boots .rvf file in < 125 ms, first query responds correctly

### Phase 5: TEE Attestation Binding (KERNEL_SEG + WITNESS_SEG)

**Duration**: 3 weeks
**Deliverables**:
- New witness type `KERNEL_ATTESTATION (0x10)` in WITNESS_SEG
- Attestation flow: kernel generates quote, verifier checks measurement chain
- SGX integration (DCAP remote attestation)
- SEV-SNP integration (guest attestation report)
- TDX integration (TD report)
- Cross-check: `KERNEL_SEG.image_hash == measured_image_in_quote`
- End-to-end test: boot in simulated TEE (SoftwareTee), verify attestation chain
- Documentation: threat model, trust boundaries, measurement lifecycle

**Preconditions**: Phase 4 complete, TEE hardware or simulation available
**Success criteria**: Full attestation chain verified in CI with SoftwareTee; manual verification on real SGX/SEV-SNP hardware

## GOAP Plan

### World State (Current — updated 2026-02-16)

```yaml
rvf_types_exists: true
rvf_wire_exists: true
rvf_manifest_exists: true
rvf_runtime_exists: true
rvf_wasm_exists: true
rvf_crypto_exists: true
segment_types_count: 23  # 0x00-0x0D, 0x0E-0x10, 0x20-0x23, 0x30-0x32
kernel_seg_defined: true        # SegmentType::Kernel = 0x0E
ebpf_seg_defined: true          # SegmentType::Ebpf = 0x0F
wasm_seg_defined: true          # SegmentType::Wasm = 0x10
kernel_header_defined: true     # KernelHeader (128B repr(C)) in kernel.rs
ebpf_header_defined: true       # EbpfHeader (64B repr(C)) in ebpf.rs
wasm_header_defined: true       # WasmHeader (64B repr(C)) in wasm_bootstrap.rs
agi_container_defined: true     # AgiContainerHeader (64B repr(C)) in agi_container.rs
domain_expansion_types: true    # TransferPrior, PolicyKernel, CostCurve segments
kernel_seg_codec: false
ebpf_seg_codec: false
hermit_kernel_built: false
ebpf_program_built: false
firecracker_launcher: false
tee_attestation_binding: false
self_booting_rvf: false
```

### Goal State

```yaml
kernel_seg_defined: true
ebpf_seg_defined: true
kernel_header_defined: true
ebpf_header_defined: true
kernel_seg_codec: true
ebpf_seg_codec: true
hermit_kernel_built: true
ebpf_program_built: true
firecracker_launcher: true
tee_attestation_binding: true
self_booting_rvf: true
```

### Actions

```
Action: define_segment_types
  Preconditions: [rvf_types_exists]
  Effects: [kernel_seg_defined, ebpf_seg_defined]
  Cost: 1

Action: define_kernel_header
  Preconditions: [kernel_seg_defined]
  Effects: [kernel_header_defined]
  Cost: 2

Action: define_ebpf_header
  Preconditions: [ebpf_seg_defined]
  Effects: [ebpf_header_defined]
  Cost: 2

Action: build_kernel_codec
  Preconditions: [kernel_header_defined, rvf_wire_exists]
  Effects: [kernel_seg_codec]
  Cost: 3

Action: build_ebpf_codec
  Preconditions: [ebpf_header_defined, rvf_wire_exists]
  Effects: [ebpf_seg_codec]
  Cost: 3

Action: build_hermit_kernel
  Preconditions: [kernel_seg_codec, rvf_manifest_exists]
  Effects: [hermit_kernel_built]
  Cost: 8

Action: build_ebpf_program
  Preconditions: [ebpf_seg_codec]
  Effects: [ebpf_program_built]
  Cost: 5

Action: build_firecracker_launcher
  Preconditions: [hermit_kernel_built, kernel_seg_codec]
  Effects: [firecracker_launcher]
  Cost: 5

Action: bind_tee_attestation
  Preconditions: [firecracker_launcher, rvf_crypto_exists]
  Effects: [tee_attestation_binding]
  Cost: 8

Action: integrate_self_boot
  Preconditions: [firecracker_launcher, ebpf_program_built, tee_attestation_binding]
  Effects: [self_booting_rvf]
  Cost: 3
```

### Critical Path (A* optimal)

```
define_segment_types (1)
  -> define_kernel_header (2)
    -> build_kernel_codec (3)
      -> build_hermit_kernel (8)
        -> build_firecracker_launcher (5)
          -> bind_tee_attestation (8)
            -> integrate_self_boot (3)

Total cost on critical path: 30

Parallel path (eBPF, runs alongside kernel path):
  define_segment_types (1)
    -> define_ebpf_header (2)
      -> build_ebpf_codec (3)
        -> build_ebpf_program (5)
          -> [joins at integrate_self_boot]
```

### Milestones

| Milestone | Phase | Success Criteria | Measurable |
|-----------|-------|-----------------|------------|
| **M1: Types defined** | 1 | `SegmentType::Kernel` and `SegmentType::Ebpf` compile, field offset tests pass | `cargo test -p rvf-types` green |
| **M2: eBPF embeds** | 2 | EBPF_SEG round-trips through codec, eBPF program loads in kernel | BPF verifier accepts program from segment |
| **M3: Hermit boots** | 3 | Hermit unikernel reads .rvf via virtio-blk, parses Level 0 manifest | Health endpoint returns 200 in QEMU |
| **M4: Firecracker serves** | 4 | `rvf launch test.rvf` boots, query returns correct nearest neighbors | recall@10 >= 0.70 within 200 ms of boot |
| **M5: TEE attested** | 5 | Attestation chain: file signature -> kernel measurement -> TEE quote verified | SoftwareTee CI test passes; manual SGX test passes |
| **M6: Production ready** | All | All tiers work, performance targets met, documentation complete | All benchmarks meet targets in CI |

## Consequences

### Benefits

1. **Zero-dependency deployment**: A single .rvf file boots and serves queries. No runtime installation, no container image pull, no package manager.
2. **Minimal TCB for confidential computing**: The kernel image is cryptographically measured and attested. The trust chain covers both data and runtime.
3. **Sub-125ms cold start**: Firecracker + unikernel eliminates the multi-second startup of traditional runtimes.
4. **Kernel-level acceleration**: eBPF hot-path queries bypass userspace entirely for cache hits, achieving sub-10 us latency.
5. **Architectural portability**: Kernel images for x86_64, aarch64, and riscv64 can coexist in the same file (multiple KERNEL_SEGs with different `arch` values).
6. **Graceful degradation**: Files with KERNEL_SEG work as pure data files for readers that do not support self-booting. The computational capability is additive.
7. **Post-quantum supply chain**: ML-DSA-65 signatures cover both data integrity and kernel integrity, providing quantum-resistant verification of the entire file.
8. **Edge computing**: Air-gapped and disconnected environments can deploy vector search by transferring a single file.

### Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Hermit kernel too large for practical embedding | Low | Medium | Budget is 2 MB; Hermit minimal builds are ~400 KB. Fallback to stripped micro-Linux. |
| Firecracker not available on target platform | Medium | Medium | Provide alternative VMMs (Cloud Hypervisor, QEMU, Uhyve). KERNEL_SEG is VMM-agnostic. |
| eBPF verifier rejects distance computation program | Low | Low | Use well-tested patterns; distance computation is a bounded loop with known iteration count. |
| TEE hardware unavailable in CI | High | Low | SoftwareTee (0xFE) platform variant for CI testing. Manual verification on real hardware. |
| Kernel image supply chain compromise | Low | Critical | Mandatory signing (ML-DSA-65). Reproducible builds. Build provenance via build_id. |
| Specification complexity delays implementation | Medium | Medium | Phased implementation; each phase is independently useful. eBPF and kernel paths are parallel. |
| WASM + eBPF + unikernel creates confusion about which tier to use | Medium | Low | Clear tier selection logic. Default to host runtime; self-boot is opt-in. |

### Migration Path

1. **No migration required**: Existing RVF files continue to work unchanged.
2. **Opt-in**: Users who want self-booting add KERNEL_SEG via the `rvf-kernel` crate.
3. **CLI tool**: `rvf embed-kernel --arch x86_64 --type hermit mydata.rvf` adds a KERNEL_SEG.
4. **Build pipeline**: CI can produce "bootable" and "data-only" variants of the same .rvf file.

## Related Decisions

- **ADR-029** (RVF canonical format): Defines the segment model, wire format, and manifest structure that KERNEL_SEG and EBPF_SEG extend.
- **ADR-005** (WASM runtime): Defines Tier 1 (WASM microkernel). KERNEL_SEG is Tier 3, complementary.
- **ADR-012** (Security remediation): Establishes the cryptographic signing and attestation framework that KERNEL_SEG reuses.
- **ADR-003** (SIMD optimization): The unikernel's distance computation kernels follow the same SIMD strategy (AVX-512, NEON, WASM v128).

## References

- [Hermit OS](https://hermit-os.org/) -- Rust-native unikernel
- [Firecracker](https://firecracker-microvm.github.io/) -- Secure microVM for serverless
- [Aya](https://aya-rs.dev/book/) -- Rust eBPF framework
- [Asterinas](https://github.com/asterinas/asterinas) -- Linux ABI-compatible Rust framekernel (USENIX ATC 2025)
- [Theseus OS](https://github.com/theseus-os/Theseus) -- Safe-language OS with intralingual design
- [WASI](https://wasi.dev/) -- WebAssembly System Interface
- [fips204 crate](https://crates.io/crates/fips204) -- Pure Rust ML-DSA-65 implementation
- [Confidential Computing Consortium](https://confidentialcomputing.io/)
- [Gramine](https://gramineproject.io/) -- SGX library OS
- [WebAssembly and Unikernels: A Comparative Study](https://arxiv.org/html/2509.09400v1) -- Serverless edge comparison
- [AppImage](https://appimage.org/) -- Self-contained Linux application format

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-14 | ruv.io | Initial proposal |
| 1.1 | 2026-02-16 | implementation review | Phase 1 complete: KernelHeader (128B), EbpfHeader (64B), WasmHeader (64B), all enums and flag constants implemented in rvf-types with 32+ tests. Updated GOAP world state. Added WASM_SEG (0x10) and domain expansion types (0x30-0x32) to segment registry. AGI container header (64B) implemented. |
