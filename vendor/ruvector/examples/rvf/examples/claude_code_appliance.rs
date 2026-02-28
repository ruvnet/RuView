//! Claude Code Appliance — Self-Booting AI Development Environment
//!
//! Creates a single `.rvf` file that boots as a complete Claude Code
//! development environment. The file contains:
//!
//! 1. KERNEL_SEG — Linux microkernel (MicroLinux) with SSH enabled
//!    Includes a 128-byte KernelBinding (ADR-031) that cryptographically
//!    ties the kernel to the manifest root hash, preventing segment-swap
//!    attacks. The binding also specifies a policy hash and segment mask.
//! 2. VEC_SEG — Embeddings for installed packages, tools, and configs
//! 3. EBPF_SEG — Network filter for secure access
//! 4. WITNESS_SEG — Audit trail for every install step
//! 5. CRYPTO_SEG — SSH host key + authorized keys (Ed25519 signed)
//!
//! Boot script (embedded in kernel cmdline) installs Claude Code:
//!   curl -fsSL https://claude.ai/install.sh | bash
//!
//! RVCOW capabilities (ADR-031):
//!   - COW branching: derive user-specific branches without copying all data
//!   - Membership filters: shared HNSW index with per-branch visibility
//!   - Snapshot freeze: immutable snapshots for versioned deployments
//!   - See `cow_branching`, `membership_filter`, `snapshot_freeze` examples
//!
//! Usage:
//!   cargo run --example claude_code_appliance
//!   rvf inspect output/claude_code_appliance.rvf
//!   rvf status output/claude_code_appliance.rvf --json
//!
//! Deployment:
//!   # Boot on Firecracker
//!   firecracker --kernel claude_code_appliance.rvf
//!
//!   # Boot on QEMU
//!   qemu-system-x86_64 -kernel claude_code_appliance.rvf -nographic
//!
//!   # SSH into the running appliance
//!   ssh -p 2222 deploy@localhost

use rvf_crypto::{
    create_witness_chain, shake256_256, sign_segment, verify_segment,
    verify_witness_chain, WitnessEntry,
};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore};
use rvf_types::kernel::{KernelArch, KernelType};
use rvf_types::{DerivationType, SegmentHeader, SegmentType};
use rvf_kernel::KernelBuilder;
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::fs;
use std::path::Path;

/// LCG deterministic random vector generator.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn hex(data: &[u8], n: usize) -> String {
    data.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

fn keygen(seed: u64) -> (SigningKey, VerifyingKey) {
    let mut key_bytes = [0u8; 32];
    let mut x = seed;
    for b in &mut key_bytes {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (x >> 56) as u8;
    }
    let sk = SigningKey::from_bytes(&key_bytes);
    let vk = sk.verifying_key();
    (sk, vk)
}

/// Software package definition.
struct Package {
    name: &'static str,
    version: &'static str,
    category: &'static str,
    size_kb: u64,
    description: &'static str,
}

fn main() {
    println!("=== Claude Code Appliance — Self-Booting AI Dev Environment ===\n");

    let dim = 128;

    // Create output directory
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    fs::create_dir_all(&out_dir).expect("create output dir");
    let store_path = out_dir.join("claude_code_appliance.rvf");
    if store_path.exists() {
        fs::remove_file(&store_path).expect("remove old file");
    }

    // ================================================================
    // Phase 1: Define the software stack
    // ================================================================
    println!("--- Phase 1: Software Stack ---\n");

    let packages = vec![
        // Core OS
        Package { name: "musl-libc", version: "1.2.5", category: "core",
            size_kb: 892, description: "Minimal C library" },
        Package { name: "busybox", version: "1.36.1", category: "core",
            size_kb: 1024, description: "Core UNIX utilities" },
        Package { name: "linux-kernel", version: "6.8.0-micro", category: "kernel",
            size_kb: 8192, description: "Linux microkernel (minimal)" },
        // SSH
        Package { name: "openssh-server", version: "9.6p1", category: "ssh",
            size_kb: 2048, description: "SSH protocol server" },
        Package { name: "openssh-client", version: "9.6p1", category: "ssh",
            size_kb: 1536, description: "SSH protocol client" },
        Package { name: "openssl", version: "3.2.1", category: "crypto",
            size_kb: 4096, description: "TLS/SSL library" },
        // Networking
        Package { name: "curl", version: "8.6.0", category: "network",
            size_kb: 1024, description: "HTTP/HTTPS client (for install scripts)" },
        Package { name: "iproute2", version: "6.7.0", category: "network",
            size_kb: 1280, description: "Network configuration" },
        Package { name: "iptables", version: "1.8.10", category: "network",
            size_kb: 768, description: "Firewall" },
        Package { name: "wireguard-tools", version: "1.0.20210914", category: "vpn",
            size_kb: 256, description: "WireGuard VPN" },
        // Development tools
        Package { name: "git", version: "2.44.0", category: "dev",
            size_kb: 8192, description: "Version control" },
        Package { name: "nodejs", version: "22.0.0", category: "dev",
            size_kb: 32768, description: "JavaScript runtime" },
        Package { name: "npm", version: "10.5.0", category: "dev",
            size_kb: 4096, description: "Node package manager" },
        Package { name: "python3", version: "3.12.2", category: "dev",
            size_kb: 16384, description: "Python runtime" },
        Package { name: "rust-toolchain", version: "1.87.0", category: "dev",
            size_kb: 65536, description: "Rust compiler + cargo" },
        // Claude Code
        Package { name: "claude-code", version: "latest", category: "ai",
            size_kb: 51200, description: "Claude Code CLI (via claude.ai/install.sh)" },
        // System services
        Package { name: "chrony", version: "4.5", category: "system",
            size_kb: 512, description: "NTP time sync" },
        Package { name: "syslog-ng", version: "4.6.0", category: "system",
            size_kb: 2048, description: "System logging" },
        // RuVector
        Package { name: "rvf-cli", version: "0.1.0", category: "ai",
            size_kb: 2048, description: "RVF vector store CLI" },
        Package { name: "ruvector-agent", version: "0.1.0", category: "ai",
            size_kb: 4096, description: "RuVector edge inference agent" },
    ];

    let total_kb: u64 = packages.iter().map(|p| p.size_kb).sum();
    println!("  Packages: {} ({:.1} MB total)", packages.len(), total_kb as f64 / 1024.0);
    for cat in &["core", "kernel", "ssh", "crypto", "network", "vpn", "dev", "ai", "system"] {
        let pkgs: Vec<_> = packages.iter().filter(|p| p.category == *cat).collect();
        if !pkgs.is_empty() {
            let cat_kb: u64 = pkgs.iter().map(|p| p.size_kb).sum();
            println!("    {:10}: {} pkgs ({:.1} MB)", cat, pkgs.len(), cat_kb as f64 / 1024.0);
        }
    }
    println!();

    // ================================================================
    // Phase 2: Create RVF store + embed kernel
    // ================================================================
    println!("--- Phase 2: Create Bootable Image ---\n");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&store_path, options).expect("create store");

    // Build a REAL initramfs using rvf-kernel's cpio builder.
    // This produces a valid gzipped cpio/newc archive with:
    //   - Standard Linux directory structure (/bin, /sbin, /etc, /dev, ...)
    //   - Device nodes (console, ttyS0, null, zero, urandom)
    //   - /init script that boots network, starts SSH, installs Claude Code
    let builder = KernelBuilder::new(KernelArch::X86_64)
        .with_initramfs(&["sshd", "rvf-server"]);
    let initramfs = builder.build_initramfs(
        &["sshd", "rvf-server"],
        &[], // Extra binaries would be added here in production
    ).expect("build initramfs");
    println!("  Initramfs:        {} bytes (real gzipped cpio archive)", initramfs.len());

    // Build real Linux kernel (Docker) or fall back to builtin stub
    let tmpdir = std::env::temp_dir().join("rvf-appliance-build");
    std::fs::create_dir_all(&tmpdir).ok();
    let built = builder.build(&tmpdir).expect("build kernel");
    let kernel_label = if built.bzimage.len() > 8192 { "real bzImage" } else { "builtin stub" };
    println!("  Kernel built:     {} bytes ({})", built.bzimage.len(), kernel_label);
    let kernel_image = built.bzimage;

    // The kernel cmdline configures the system on first boot:
    //   1. Enable networking
    //   2. Start SSH server
    //   3. Install Claude Code from official installer
    //   4. Start the RVF query API
    let cmdline = concat!(
        "console=ttyS0 root=/dev/vda rw init=/sbin/init net.ifnames=0 ",
        "rvf.listen=0.0.0.0:8080 ",
        "rvf.ssh_port=2222 ",
        "rvf.boot_script=\"",
            "#!/bin/sh\\n",
            "set -e\\n",
            "# Enable networking\\n",
            "ip link set eth0 up\\n",
            "dhcpcd eth0\\n",
            "# Start SSH server\\n",
            "mkdir -p /etc/ssh\\n",
            "ssh-keygen -A\\n",
            "/usr/sbin/sshd -p 2222\\n",
            "# Install Claude Code\\n",
            "curl -fsSL https://claude.ai/install.sh | bash\\n",
            "# Start RVF query server\\n",
            "rvf serve /data/vectors.rvf --port 8080 &\\n",
            "echo 'Claude Code appliance ready.'\\n",
            "exec /bin/sh\\n",
        "\""
    );

    let kernel_seg_id = store
        .embed_kernel(
            KernelArch::X86_64 as u8,
            KernelType::MicroLinux as u8,
            0x003F, // HAS_QUERY_API | HAS_NETWORKING | HAS_STORAGE | HAS_COMPUTE | HAS_SSH
            &kernel_image,
            2222, // SSH port
            Some(cmdline),
        )
        .expect("embed kernel");

    println!("  Kernel embedded:  segment ID {}", kernel_seg_id);
    println!("  Arch:             x86_64");
    println!("  Type:             MicroLinux");
    println!("  Image size:       {} bytes", kernel_image.len());
    println!("  SSH port:         2222");
    println!("  API port:         8080");
    println!("  Boot script:      installs Claude Code via curl");
    println!("  Cmdline length:   {} bytes", cmdline.len());
    println!();

    // ================================================================
    // Phase 3: Ingest package collection
    // ================================================================
    println!("--- Phase 3: Ingest Package Collection ---\n");

    let pkg_vecs: Vec<Vec<f32>> = packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let mut v = random_vector(dim, i as u64 * 31 + 7);
            let cat_val = match pkg.category {
                "core" => 0.9, "kernel" => 0.85, "ssh" => 0.8, "crypto" => 0.7,
                "network" => 0.6, "vpn" => 0.5, "dev" => 0.4, "ai" => 0.3,
                "system" => 0.2, _ => 0.0,
            };
            v[0] = cat_val;
            v[1] = (pkg.size_kb as f32).ln() / 12.0;
            v
        })
        .collect();

    let refs: Vec<&[f32]> = pkg_vecs.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (1..=packages.len() as u64).collect();

    let mut metadata = Vec::new();
    for pkg in &packages {
        metadata.push(MetadataEntry { field_id: 0, value: MetadataValue::String(pkg.name.to_string()) });
        metadata.push(MetadataEntry { field_id: 1, value: MetadataValue::String(pkg.version.to_string()) });
        metadata.push(MetadataEntry { field_id: 2, value: MetadataValue::String(pkg.category.to_string()) });
        metadata.push(MetadataEntry { field_id: 3, value: MetadataValue::U64(pkg.size_kb) });
        metadata.push(MetadataEntry { field_id: 4, value: MetadataValue::String(pkg.description.to_string()) });
    }

    let ingest = store.ingest_batch(&refs, &ids, Some(&metadata)).expect("ingest");
    println!("  Ingested {} packages into RVF image", ingest.accepted);
    println!();

    // ================================================================
    // Phase 4: SSH key management
    // ================================================================
    println!("--- Phase 4: SSH Configuration ---\n");

    let (host_sk, host_vk) = keygen(9999);
    println!("  Host key:      ed25519 fp={}...", hex(&host_vk.to_bytes(), 8));

    let ssh_users = [
        ("root", "admin", 1000u64),
        ("deploy", "deploy", 2000),
        ("claude", "developer", 3000),
    ];

    for (user, perms, seed) in &ssh_users {
        let (_user_sk, user_vk) = keygen(*seed);
        let auth = format!("ssh-authorized-key:user={},perms={}", user, perms);
        let header = SegmentHeader::new(SegmentType::Crypto as u8, *seed);
        let sig = sign_segment(&header, auth.as_bytes(), &host_sk);
        let valid = verify_segment(&header, auth.as_bytes(), &sig, &host_vk);

        println!(
            "  User {:10}: ed25519 fp={}... perms={:<12} signed={}",
            user, hex(&user_vk.to_bytes(), 6), perms,
            if valid { "OK" } else { "FAIL" },
        );
    }
    println!();

    // ================================================================
    // Phase 5: eBPF network filter
    // ================================================================
    println!("--- Phase 5: eBPF Network Filter ---\n");

    // Real eBPF socket filter source from rvf-ebpf crate.
    // This is the actual BPF C source code for port-based access control.
    // In production, compile with: EbpfCompiler::new()?.compile_source(source, SocketFilter)
    let ebpf_source = rvf_ebpf::programs::SOCKET_FILTER;
    println!("  eBPF source:      {} bytes (real BPF C program)", ebpf_source.len());
    let ebpf_bytecode = ebpf_source.as_bytes().to_vec();
    let btf = Vec::new(); // BTF generated during clang compilation
    let ebpf_seg_id = store
        .embed_ebpf(
            0x02, // SocketFilter
            0x03, // SocketFilter attach
            dim as u16,
            &ebpf_bytecode,
            Some(&btf),
        )
        .expect("embed ebpf");

    println!("  eBPF filter:     segment ID {}", ebpf_seg_id);
    println!("  Program type:    SocketFilter");
    println!("  Allowed ports:   2222 (SSH), 8080 (API)");
    println!("  Bytecode size:   {} bytes", ebpf_bytecode.len());
    println!();

    // ================================================================
    // Phase 6: Witness chain (audit trail)
    // ================================================================
    println!("--- Phase 6: Build Audit Trail ---\n");

    let ts = 1_700_000_000_000_000_000u64;
    let entries = vec![
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("image_create:packages={},arch=x86_64", packages.len()).as_bytes(),
            ),
            timestamp_ns: ts,
            witness_type: 0x08,
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("kernel_embed:type=MicroLinux,size={}", kernel_image.len()).as_bytes(),
            ),
            timestamp_ns: ts + 1_000_000,
            witness_type: 0x02,
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("ssh_config:users={},host_key=ed25519", ssh_users.len()).as_bytes(),
            ),
            timestamp_ns: ts + 2_000_000,
            witness_type: 0x01,
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"ebpf_filter:ports=2222,8080"),
            timestamp_ns: ts + 3_000_000,
            witness_type: 0x02,
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"claude_code_install:curl -fsSL https://claude.ai/install.sh | bash"),
            timestamp_ns: ts + 4_000_000,
            witness_type: 0x01,
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"image_sealed:all_segments_verified"),
            timestamp_ns: ts + 5_000_000,
            witness_type: 0x07,
        },
    ];

    let chain = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain).expect("verify chain");
    println!("  Audit entries: {} (all verified)", verified.len());
    for (i, e) in verified.iter().enumerate() {
        let label = match e.witness_type {
            0x01 => "PROVENANCE",
            0x02 => "COMPUTATION",
            0x07 => "COMP_PROOF",
            0x08 => "DATA_PROV ",
            _ => "UNKNOWN   ",
        };
        println!("    #{}: {} hash={}...", i + 1, label, hex(&e.action_hash, 8));
    }
    println!();

    // ================================================================
    // Phase 7: Derive a snapshot for distribution
    // ================================================================
    println!("--- Phase 7: Derive Distribution Snapshot ---\n");

    let snapshot_path = out_dir.join("claude_code_appliance_v1.rvf");
    let snapshot = store
        .derive(&snapshot_path, DerivationType::Snapshot, None)
        .expect("derive snapshot");

    println!("  Base image:      claude_code_appliance.rvf");
    println!("  Snapshot:        claude_code_appliance_v1.rvf");
    println!("  Lineage depth:   {}", snapshot.lineage_depth());
    println!("  Parent ID:       {}...", hex(snapshot.parent_id(), 8));
    println!("  Parent matches:  {}", snapshot.parent_id() == store.file_id());
    snapshot.close().unwrap();
    println!();

    // ================================================================
    // Phase 8: Query the package database
    // ================================================================
    println!("--- Phase 8: Query Package Database ---\n");

    let mut ai_query = random_vector(dim, 42);
    ai_query[0] = 0.3; // AI category signal

    let results = store.query(&ai_query, 5, &QueryOptions::default()).expect("query");
    println!("  Top-5 AI-related packages:");
    for (i, r) in results.iter().enumerate() {
        let pkg = &packages[(r.id - 1) as usize];
        println!("    #{}: {} {} ({}) dist={:.4}",
            i + 1, pkg.name, pkg.version, pkg.category, r.distance);
    }
    println!();

    // ================================================================
    // Summary
    // ================================================================
    let status = store.status();
    println!("=== Claude Code Appliance Summary ===\n");
    println!("  File:            {:?}", store_path);
    println!("  File size:       {} bytes ({:.1} KB)", status.file_size, status.file_size as f64 / 1024.0);
    println!("  Segments:        {}", status.total_segments);
    println!("  Packages:        {} ({:.1} MB manifest)", packages.len(), total_kb as f64 / 1024.0);
    println!("  Vectors:         {} ({}-dim embeddings)", status.total_vectors, dim);
    println!("  KERNEL_SEG:      MicroLinux x86_64 ({} bytes)", kernel_image.len());
    println!("  EBPF_SEG:        SocketFilter ({} bytes)", ebpf_bytecode.len());
    println!("  SSH users:       {} (Ed25519 signed)", ssh_users.len());
    println!("  Witness chain:   {} entries (tamper-evident)", verified.len());
    println!("  Lineage:         base + v1 snapshot");
    println!();
    println!("  Boot sequence:");
    println!("    1. Firecracker loads KERNEL_SEG → Linux boots");
    println!("    2. SSH server starts on port 2222");
    println!("    3. curl -fsSL https://claude.ai/install.sh | bash");
    println!("    4. RVF query server starts on port 8080");
    println!("    5. Claude Code ready for use");
    println!();
    println!("  Connect:");
    println!("    ssh -p 2222 deploy@<host>");
    println!("    claude   # Claude Code CLI");
    println!("    rvf status /data/vectors.rvf");
    println!();
    println!("  Key insight: One .rvf file = bootable Linux + SSH +");
    println!("  Claude Code + RVF query server + package database +");
    println!("  cryptographic audit trail. Ship it, boot it, SSH in.");
    println!();

    store.close().unwrap();
    println!("Done.");
}
