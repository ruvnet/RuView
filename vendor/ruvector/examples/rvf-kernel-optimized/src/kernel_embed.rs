//! Linux kernel + eBPF embedding into an RVF container.

use anyhow::{anyhow, Result};
use rvf_kernel::KernelBuilder;
use rvf_runtime::RvfStore;
use rvf_types::ebpf::EbpfProgramType;
use tracing::info;

/// Result of embedding a kernel and eBPF programs into the RVF store.
pub struct KernelEmbedResult {
    /// Size of the kernel image in bytes.
    pub kernel_size: usize,
    /// Number of eBPF programs embedded.
    pub ebpf_programs: usize,
    /// SHA3-256 hash of the kernel image.
    pub kernel_hash: [u8; 32],
    /// Kernel cmdline used.
    pub cmdline: String,
}

/// Embed an optimized Linux kernel and precompiled eBPF programs into the store.
///
/// Uses `from_builtin_minimal()` for a 4KB kernel stub that works without
/// Docker or a cross-compiler. In production, replace with a real kernel
/// built via `KernelBuilder::build_docker()`.
pub fn embed_optimized_kernel(
    store: &mut RvfStore,
    cmdline: &str,
    enable_ebpf: bool,
    max_dim: u16,
) -> Result<KernelEmbedResult> {
    // Stage 1: Build minimal kernel (4KB stub, always works)
    let kernel =
        KernelBuilder::from_builtin_minimal().map_err(|e| anyhow!("kernel build: {e:?}"))?;
    let kernel_size = kernel.bzimage.len();
    let kernel_hash = kernel.image_hash;

    info!(size = kernel_size, "built minimal kernel image");

    // Stage 2: Embed kernel with optimized cmdline
    // arch=0 (x86_64), kernel_type=0 (MicroLinux), flags include COMPRESSED + VIRTIO
    let kernel_flags = 0x01 | 0x02 | 0x04; // COMPRESSED | VIRTIO_NET | VIRTIO_BLK
    store
        .embed_kernel(0, 0, kernel_flags, &kernel.bzimage, 8080, Some(cmdline))
        .map_err(|e| anyhow!("embed kernel: {e:?}"))?;

    info!("embedded kernel into RVF store");

    // Stage 3: Embed precompiled eBPF programs
    let mut ebpf_count = 0;
    if enable_ebpf {
        let programs = [
            (EbpfProgramType::XdpDistance, 1u8, 1u8),
            (EbpfProgramType::SocketFilter, 3u8, 3u8),
            (EbpfProgramType::TcFilter, 2u8, 2u8),
        ];

        for (prog_type, seg_type, attach_type) in &programs {
            let compiled = rvf_ebpf::EbpfCompiler::from_precompiled(*prog_type)
                .map_err(|e| anyhow!("ebpf compile: {e:?}"))?;
            store
                .embed_ebpf(
                    *seg_type,
                    *attach_type,
                    max_dim,
                    &compiled.elf_bytes,
                    compiled.btf_bytes.as_deref(),
                )
                .map_err(|e| anyhow!("embed ebpf: {e:?}"))?;
            ebpf_count += 1;
        }
        info!(count = ebpf_count, "embedded eBPF programs");
    }

    Ok(KernelEmbedResult {
        kernel_size,
        ebpf_programs: ebpf_count,
        kernel_hash,
        cmdline: cmdline.to_string(),
    })
}
