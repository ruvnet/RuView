# rvf-kernel

Real Linux microkernel builder for RVF cognitive containers.

## What It Does

`rvf-kernel` builds production-grade Linux kernel images and initramfs archives for embedding into `.rvf` files. A single `.rvf` file with `KERNEL_SEG` boots as a standalone Linux microservice on QEMU, Firecracker, or bare metal.

## Features

| Feature | Description |
|---------|-------------|
| **KernelBuilder** | Builds bzImage from source via Docker, or loads prebuilt images |
| **Initramfs builder** | Real cpio/newc format archives with gzip compression |
| **Docker pipeline** | Reproducible kernel compilation with Linux 6.8.x config |
| **SHA3-256 verification** | Cryptographic hash verification of kernel artifacts |
| **KernelVerifier** | Extract and verify kernels from KERNEL_SEG |

## Usage

```rust
use rvf_kernel::KernelBuilder;
use rvf_types::kernel::KernelArch;

// Option 1: Load a prebuilt kernel
let kernel = KernelBuilder::from_prebuilt("bzImage")?;

// Option 2: Build in Docker (reproducible)
let builder = KernelBuilder::new(KernelArch::X86_64)
    .kernel_version("6.8.12")
    .with_initramfs(&["sshd", "rvf-server"]);
let kernel = builder.build_docker(&context_dir)?;

// Option 3: Build just the initramfs
let initramfs = builder.build_initramfs(
    &["sshd", "rvf-server"],
    &[("rvf-server", &binary_bytes)],
)?;
```

## Kernel Config Highlights

- VirtIO PCI/BLK/NET for VM I/O
- BPF + JIT for eBPF programs
- KASLR + stack protector for security
- No modules, USB, DRM, or wireless (minimal attack surface)
- ~1.5 MB bzImage, ~512 KB initramfs

## Tests

```bash
cargo test -p rvf-kernel  # 37 tests
```

## License

MIT OR Apache-2.0
